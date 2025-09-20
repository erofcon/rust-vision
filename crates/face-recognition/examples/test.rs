use anyhow::Result;
use ndarray::{Array, ArrayBase, Dim, Ix, IxDyn, OwnedRepr};
use opencv::core::{Mat, MatTraitConst, Point, Point2f, Rect, Scalar, Size};
use opencv::{imgcodecs, imgproc};
use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider};
use ort::inputs;
use ort::session::{Session, SessionOutputs};

static MODEL_WH: i32 = 800;
const CONFIDENCE_THRESHOLD: f32 = 0.45;
const NMS_THRESHOLD: f32 = 0.45;

#[derive(Debug, Clone)]
struct Detection {
    bbox: [f32; 4],
    confidence: f32,
    keypoints: Vec<[f32; 3]>,
    yaw: f32,
    pitch: f32,
    roll: f32,
}

fn load_model(detection_model_path: &str) -> Result<Session> {
    ort::init()
        .with_execution_providers([
            CUDAExecutionProvider::default().build().error_on_failure(),
            CPUExecutionProvider::default().build().error_on_failure(),
        ])
        .commit()?;

    Ok(Session::builder()?
        .with_inter_threads(4)?
        .commit_from_file(detection_model_path)?)
}

fn resize_with_padding(src: &Mat) -> Result<(Mat, f32, f32, f32)> {
    let src_h = src.rows() as f32;
    let src_w = src.cols() as f32;
    let target_size = MODEL_WH as f32;
    let scale = (target_size / src_w).min(target_size / src_h);

    let new_w = (src_w * scale) as i32;
    let new_h = (src_h * scale) as i32;

    let mut resized = Mat::default();
    opencv::imgproc::resize(
        src,
        &mut resized,
        Size::new(new_w, new_h),
        0.0,
        0.0,
        opencv::imgproc::INTER_LINEAR,
    )?;

    let mut padded = Mat::default();
    opencv::core::copy_make_border(
        &resized,
        &mut padded,
        (MODEL_WH - new_h) / 2,
        (MODEL_WH - new_h) / 2,
        (MODEL_WH - new_w) / 2,
        (MODEL_WH - new_w) / 2,
        opencv::core::BORDER_CONSTANT,
        Scalar::new(114.0, 114.0, 114.0, 0.0),
    )?;

    let pad_x = ((MODEL_WH - new_w) / 2) as f32;
    let pad_y = ((MODEL_WH - new_h) / 2) as f32;

    Ok((padded, scale, pad_x, pad_y))
}

fn resize(img: &Mat, target_width: i32, target_height: i32) -> Result<Mat> {
    let mut resized = Mat::default();
    imgproc::resize(
        img,
        &mut resized,
        Size::new(target_width, target_height),
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    )?;

    Ok(resized)
}
fn prepare_img(img: &Mat) -> Result<ArrayBase<OwnedRepr<f32>, Dim<[Ix; 4]>>> {
    let mut model_input = Array::zeros((1, 3, MODEL_WH as usize, MODEL_WH as usize));

    let height = img.rows().min(MODEL_WH);
    let width = img.cols().min(MODEL_WH);

    for y in 0..height {
        for x in 0..width {
            let pixel = img.at_2d::<opencv::core::Vec3b>(y, x)?;
            model_input[[0, 0, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
            model_input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
            model_input[[0, 2, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
        }
    }
    Ok(model_input)
}

fn inference(
    session: Session,
    model_input: ArrayBase<OwnedRepr<f32>, Dim<[Ix; 4]>>,
) -> Result<Array<f32, IxDyn>> {
    let outputs: SessionOutputs = session.run(inputs!["images" => model_input]?)?;
    let output = outputs["output0"].try_extract_tensor::<f32>()?.into_owned();
    Ok(output)
}

fn draw_detections(img: &mut Mat, detections: &[Detection]) -> Result<()> {
    for (idx, detection) in detections.iter().enumerate() {
        // Зеленый для фронтальных, красный для профильных
        let color = Scalar::new(0.0, 255.0, 0.0, 0.0); // Зеленый

        // Рамка
        let pt1 = Point::new(detection.bbox[0] as i32, detection.bbox[1] as i32);
        let pt2 = Point::new(detection.bbox[2] as i32, detection.bbox[3] as i32);
        imgproc::rectangle(
            img,
            opencv::core::Rect::from_points(pt1, pt2),
            color,
            2,
            imgproc::LINE_8,
            0,
        )?;

        // Подпись
        let label = format!(
            "{}: {:.2} | {} | Y:°",
            idx + 1,
            detection.confidence,
            detection.yaw
        );

        imgproc::put_text(
            img,
            &label,
            Point::new(detection.bbox[0] as i32, detection.bbox[1] as i32 - 10),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            color,
            1,
            imgproc::LINE_8,
            false,
        )?;
    }
    Ok(())
}

fn calculate_head_pose(keypoints: &[[f32; 3]]) -> (f32, f32, f32) {
    if keypoints.len() < 5
        || keypoints[0][2] < 0.3
        || keypoints[1][2] < 0.3
        || keypoints[2][2] < 0.3
    {
        return (0.0, 0.0, 0.0);
    }

    let [left_eye, right_eye, nose, _left_mouth, _right_mouth] = [
        keypoints[0],
        keypoints[1],
        keypoints[2],
        keypoints[3],
        keypoints[4],
    ];

    // YAW = горизонтальное смещение носа
    let eye_center_x = (left_eye[0] + right_eye[0]) / 2.0;
    let nose_offset_x = nose[0] - eye_center_x;
    let yaw = (nose_offset_x * 0.5).clamp(-90.0, 90.0);

    // PITCH = вертикальное смещение носа
    let eye_center_y = (left_eye[1] + right_eye[1]) / 2.0;
    let nose_offset_y = nose[1] - eye_center_y;
    let pitch = (nose_offset_y * 0.3).clamp(-45.0, 45.0);

    // ROLL = угол наклона линии глаз
    let dx = right_eye[0] - left_eye[0];
    let dy = right_eye[1] - left_eye[1];
    let roll = (dy.atan2(dx) * 180.0 / std::f32::consts::PI).clamp(-45.0, 45.0);

    (yaw, pitch, roll)
}

fn process_detections(
    outputs: &Array<f32, IxDyn>,
    original_width: i32,
    original_height: i32,
    scale: f32,
    pad_x: f32,
    pad_y: f32,
) -> Result<Vec<Detection>> {
    let detections = Vec::new();
    let shape = outputs.shape();

    if shape.len() != 3 {
        return Ok(detections);
    }

    let (_, num_features, num_detections) = (shape[0], shape[1], shape[2]);
    let mut candidates = Vec::new();

    for i in 0..num_detections {
        let confidence = outputs[[0, 4, i]];
        if confidence <= CONFIDENCE_THRESHOLD {
            continue;
        }

        let cx = outputs[[0, 0, i]];
        let cy = outputs[[0, 1, i]];
        let w = outputs[[0, 2, i]];
        let h = outputs[[0, 3, i]];

        let x_center = (cx - pad_x) / scale;
        let y_center = (cy - pad_y) / scale;
        let width = w / scale;
        let height = h / scale;

        let x1 = x_center - width / 2.0;
        let y1 = y_center - height / 2.0;
        let x2 = x_center + width / 2.0;
        let y2 = y_center + height / 2.0;

        if x1 >= 0.0
            && y1 >= 0.0
            && x2 <= original_width as f32
            && y2 <= original_height as f32
            && x2 > x1
            && y2 > y1
            && width > 10.0
            && height > 10.0
        {
            let bbox = [x1, y1, x2, y2];
            let mut keypoints = Vec::new();

            // Извлекаем ключевые точки
            for j in 0..5 {
                let keypoint_base = 5 + j * 3;
                if keypoint_base + 2 < num_features {
                    let kp_x = (outputs[[0, keypoint_base, i]] - pad_x) / scale;
                    let kp_y = (outputs[[0, keypoint_base + 1, i]] - pad_y) / scale;
                    let kp_visibility = outputs[[0, keypoint_base + 2, i]];
                    keypoints.push([kp_x, kp_y, kp_visibility]);
                }
            }

            let (yaw, pitch, roll) = calculate_head_pose(&keypoints);

            candidates.push(Detection {
                bbox,
                confidence,
                keypoints,
                yaw,
                pitch,
                roll,
            });
        }
    }

    // NMS
    candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
    apply_nms(candidates)
}

fn apply_nms(detections: Vec<Detection>) -> Result<Vec<Detection>> {
    let mut keep = Vec::new();
    let mut suppressed = vec![false; detections.len()];

    for i in 0..detections.len() {
        if suppressed[i] {
            continue;
        }
        keep.push(detections[i].clone());

        for j in (i + 1)..detections.len() {
            if !suppressed[j]
                && calculate_iou(&detections[i].bbox, &detections[j].bbox) > NMS_THRESHOLD
            {
                suppressed[j] = true;
            }
        }
    }
    Ok(keep)
}

fn calculate_iou(bbox1: &[f32; 4], bbox2: &[f32; 4]) -> f32 {
    let x1 = bbox1[0].max(bbox2[0]);
    let y1 = bbox1[1].max(bbox2[1]);
    let x2 = bbox1[2].min(bbox2[2]);
    let y2 = bbox1[3].min(bbox2[3]);

    if x2 <= x1 || y2 <= y1 {
        return 0.0;
    }

    let intersection = (x2 - x1) * (y2 - y1);
    let area1 = (bbox1[2] - bbox1[0]) * (bbox1[3] - bbox1[1]);
    let area2 = (bbox2[2] - bbox2[0]) * (bbox2[3] - bbox2[1]);
    let union = area1 + area2 - intersection;

    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn align_face(face: &Mat, detection: &Detection) -> Result<Mat> {
    let mut left_eye: Option<Point2f> = None;
    let mut right_eye: Option<Point2f> = None;

    let bbox = &detection.bbox;
    let face_x_min = bbox[0];
    let face_y_min = bbox[1];

    if detection.keypoints.len() >= 2 {
        if detection.keypoints[0][2] > 0.5 {
            left_eye = Some(Point2f::new(
                detection.keypoints[0][0] - face_x_min,
                detection.keypoints[0][1] - face_y_min,
            ));
        }
        if detection.keypoints[1][2] > 0.5 {
            right_eye = Some(Point2f::new(
                detection.keypoints[1][0] - face_x_min,
                detection.keypoints[1][1] - face_y_min,
            ));
        }
    }

    let mut aligned = Mat::default();

    if let (Some(left), Some(right)) = (left_eye, right_eye) {
        let dx = right.x - left.x;
        let dy = right.y - left.y;
        let angle = dy.atan2(dx) * 180.0 / std::f32::consts::PI;

        let center = Point2f::new((left.x + right.x) / 2.0, (left.y + right.y) / 2.0);
        let rotation_matrix = opencv::imgproc::get_rotation_matrix_2d(center, angle as f64, 1.0)?;

        imgproc::warp_affine(
            face,
            &mut aligned,
            &rotation_matrix,
            face.size()?,
            opencv::imgproc::INTER_LINEAR,
            opencv::core::BORDER_REFLECT_101,
            opencv::core::Scalar::default(),
        )?;
    } else {
        face.copy_to(&mut aligned)?;
    }

    Ok(aligned)
}

fn crop_detection(input_img: &Mat, detection: &Detection) -> Result<Mat> {
    let [x_min, y_min, x_max, y_max] = detection.bbox;

    let x = x_min.max(0.0) as i32;
    let y = y_min.max(0.0) as i32;
    let width = (x_max - x_min).min(input_img.cols() as f32 - x as f32) as i32;
    let height = (y_max - y_min).min(input_img.rows() as f32 - y as f32) as i32;

    let rect = Rect::new(x, y, width, height);
    let roi = Mat::roi(input_img, rect)?;

    let mut cropped = Mat::default();
    roi.copy_to(&mut cropped)?;

    Ok(cropped)
}

fn main() -> Result<()> {
    let session = load_model("models/yolo11s-pose_widerface.onnx")?;
    let img = imgcodecs::imread("assets/test/img.png", imgcodecs::IMREAD_COLOR)?;
    let face_recognition_model = load_model("models/face-recognition.onnx")?;

    let original_width = img.cols();
    let original_height = img.rows();

    let (resized, scale, pad_x, pad_y) = resize_with_padding(&img)?;

    println!("Resizing of {:?} to {:?}, {:?}", scale, pad_x, pad_y);

    let prepared_img = prepare_img(&resized)?;

    let session_outputs = inference(session, prepared_img)?;

    let detections = process_detections(
        &session_outputs,
        original_width,
        original_height,
        scale,
        pad_x,
        pad_y,
    )?;

    for (i, detection) in detections.iter().enumerate() {
        println!(
            "Лицо {}: | Conf: {:.3} | Yaw: {:.1}° | Pitch: {:.1}° | Roll: {:.1}°",
            i + 1,
            detection.confidence,
            detection.yaw,
            detection.pitch,
            detection.roll
        );

        let face = crop_detection(&img, &detection)?;
        let aligned_face = align_face(&face, detection)?;

        let recognition_face = resize(&aligned_face, 160, 160)?;

        opencv::highgui::imshow(&format!("Original Face {}", i + 1), &face)?;
        opencv::highgui::imshow(&format!("Aligned Face {}", i + 1), &aligned_face)?;
        opencv::highgui::imshow(&format!("Recognition Face {}", i + 1), &recognition_face)?;
    }

    let mut result_img = img.clone();
    draw_detections(&mut result_img, &detections)?;

    opencv::highgui::imshow("Face Detection: Green=Frontal, Red=Profile", &result_img)?;
    opencv::highgui::wait_key(0)?;

    Ok(())
}
