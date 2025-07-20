use anyhow::Result;
use ndarray::{Array, IxDyn};
use opencv::{core::{Mat, Point2f, Scalar, Size, Rect}, imgproc, imgcodecs, highgui};
use opencv::core::{MatTraitConst, Point};
use ort::{session::Session, inputs};
use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider};

const MODEL_WH: i32 = 800;
const CONF_THRESH: f32 = 0.45;
const NMS_THRESH: f32 = 0.45;
const MAX_PITCH: f32 = 10.0; // discard if pitch > 10° absolute

#[derive(Debug)]
struct HeadPose { yaw: f32, pitch: f32, roll: f32 }

fn load_model(path: &str) -> Result<Session> {
    ort::init()
        .with_execution_providers([
            CUDAExecutionProvider::default().build()?,
            CPUExecutionProvider::default().build()?
        ])?
        .commit()?;
    Ok(Session::builder()?.commit_from_file(path)?)
}

fn preprocess(img: &Mat) -> Result<(Mat, Array<f32, IxDyn>, f32, f32)> {
    let (h, w) = (img.rows() as f32, img.cols() as f32);
    let scale = (MODEL_WH as f32 / w).min(MODEL_WH as f32 / h);

    let new_w = (w * scale) as i32;
    let new_h = (h * scale) as i32;

    let mut resized = Mat::default();
    imgproc::resize(img, &mut resized, Size::new(new_w, new_h), 0.0, 0.0, imgproc::INTER_LINEAR)?;

    let pad_x = ((MODEL_WH - new_w) / 2) as i32;
    let pad_y = ((MODEL_WH - new_h) / 2) as i32;
    let mut padded = Mat::default();
    opencv::core::copy_make_border(
        &resized, &mut padded,
        pad_y, pad_y, pad_x, pad_x,
        opencv::core::BORDER_CONSTANT,
        Scalar::new(114.0, 114.0, 114.0, 0.0)
    )?;

    let mut input = Array::zeros((1, 3, MODEL_WH as usize, MODEL_WH as usize));
    for y in 0..MODEL_WH {
        for x in 0..MODEL_WH {
            let px = padded.at_2d::<opencv::core::Vec3b>(y, x)?;
            input[[0,0,y as usize,x as usize]] = px[2] as f32 / 255.0;
            input[[0,1,y as usize,x as usize]] = px[1] as f32 / 255.0;
            input[[0,2,y as usize,x as usize]] = px[0] as f32 / 255.0;
        }
    }
    Ok((padded, input.into_dyn(), scale, pad_x as f32))
}

fn postprocess(
    output: &Array<f32, IxDyn>,
    orig: &mut Mat,
    scale: f32,
    pad: f32
) -> Result<()> {
    let (ow, oh) = (orig.cols() as f32, orig.rows() as f32);
    let shape = output.shape();
    let (_, feat, dets) = (shape[0], shape[1], shape[2]);

    let mut faces = Vec::new();
    for i in 0..dets {
        let conf = output[[0,4,i]];
        if conf < CONF_THRESH { continue; }
        let cx = (output[[0,0,i]] - pad)/scale;
        let cy = (output[[0,1,i]] - pad)/scale;
        let w = output[[0,2,i]]/scale;
        let h = output[[0,3,i]]/scale;
        let x1 = (cx - w/2.0).max(0.0);
        let y1 = (cy - h/2.0).max(0.0);
        let x2 = (cx + w/2.0).min(ow);
        let y2 = (cy + h/2.0).min(oh);
        // skip invalid
        if x2<=x1 || y2<=y1 { continue; }

        // extract 5 keypoints if present
        let mut kps = Vec::new();
        if feat >= 20 {
            for j in 0..5 {
                let bx = (output[[0,5+j*3,i]] - pad)/scale;
                let by = (output[[0,6+j*3,i]] - pad)/scale;
                let vis = output[[0,7+j*3,i]];
                kps.push((bx,by,vis));
            }
        }
        // compute head pose
        if let Some(p) = calc_pose(&kps) {
            if p.pitch.abs() > MAX_PITCH { continue; }

            faces.push((x1,y1,x2,y2,conf,p));
        }
    }
    // NMS
    let final_faces = nms(faces);

    for (i,(x1,y1,x2,y2,conf,pose)) in final_faces.iter().enumerate() {
        let rect = Rect::new(*x1 as i32, *y1 as i32, (*x2 - *x1) as i32, (*y2 - *y1) as i32);
        imgproc::rectangle(orig, rect, Scalar::new(0.,255.,0.,0.), 2, imgproc::LINE_8,0)?;
        let txt = format!("{}:{:.2} Y:{:.1} P:{:.1} R:{:.1}", i+1, conf, pose.yaw, pose.pitch, pose.roll);
        // imgproc::put_text(orig, &txt, Point::new(*x1, (*y1 - 5.0) as i32).to::<i32>(), imgproc::FONT_HERSHEY_SIMPLEX, 0.5, Scalar::new(255., 255., 255., 0.), 1, imgproc::LINE_8, false)?;
    }
    highgui::imshow("Detections", orig)?;
    highgui::wait_key(0)?;
    Ok(())
}

fn calc_pose(kps: &[(f32,f32,f32)]) -> Option<HeadPose> {
    if kps.len()<5 { return None; }
    let (le, re, no, lm, rm) = (kps[0],kps[1],kps[2],kps[3],kps[4]);
    if le.2<0.3||re.2<0.3||no.2<0.3 { return None; }
    let dx = re.0 - le.0; let dy = re.1 - le.1;
    let roll = dy.atan2(dx).to_degrees().clamp(-45.0,45.0);
    let cen_x = (le.0+re.0)/2.0;
    let dist = (dx*dx + dy*dy).sqrt();
    let yaw = ((no.0-cen_x)/dist * 60.0).clamp(-90.0,90.0);
    let cen_y = (le.1+re.1)/2.0;
    let pitch = ((no.1-cen_y)/dist * 30.0).clamp(-45.0,45.0);
    Some(HeadPose{yaw,pitch,roll})
}

fn nms(mut v: Vec<(f32,f32,f32,f32,f32,HeadPose)>) -> Vec<(f32,f32,f32,f32,f32,HeadPose)> {
    v.sort_by(|a,b| b.4.partial_cmp(&a.4).unwrap());
    let mut out = Vec::new();
    while let Some(cur) = v.pop() {
        out.push(cur.clone());
        v.retain(|other| iou(&cur,other) < NMS_THRESH);
    }
    out
}

fn iou(a: &(f32,f32,f32,f32,f32,HeadPose), b: &(f32,f32,f32,f32,f32,HeadPose)) -> f32 {
    let (x1,y1,x2,y2,_,_) = *a;
    let (x1b,y1b,x2b,y2b,_,_) = *b;
    let xi1 = x1.max(x1b); let yi1 = y1.max(y1b);
    let xi2 = x2.min(x2b); let yi2 = y2.min(y2b);
    if xi2<=xi1||yi2<=yi1 { return 0.; }
    let inter = (xi2-xi1)*(yi2-yi1);
    let union = (x2-x1)*(y2-y1) + (x2b-x1b)*(y2b-y1b) - inter;
    inter/union
}

fn main() -> Result<()> {
    let img = imgcodecs::imread("assets/test/img_4.png", imgcodecs::IMREAD_COLOR)?;
    let sess = load_model("models/yolo11s-pose_widerface.onnx")?;
    let (pad, inp, sc, px) = preprocess(&img)?;
    let out = sess.run(inputs!("images"=>inp)?)?;
    let tensor = out["output0"].try_extract_tensor::<f32>()?.into_owned();
    postprocess(&tensor.into_dyn(), &mut img.clone(), sc, px)?;
    Ok(())
}
