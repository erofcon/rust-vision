// use anyhow::Result;
// use opencv::core::{Mat, Rect, Size, Vector};
// use opencv::hub_prelude::CascadeClassifierTrait;
// use opencv::objdetect::{CASCADE_SCALE_IMAGE, CascadeClassifier};
// use opencv::*;
//
// fn main() -> Result<()> {
//     let img_path = "assets/test/img.png";
//     let img1_path = "assets/test/img_1.png";
//
//     let mut img = imgcodecs::imread(&img_path.to_string(), imgcodecs::IMREAD_COLOR).unwrap();
//
//     let mut face_detector =
//         CascadeClassifier::new("assets/haara/haarcascade_frontalface_alt.xml")?;
//     let eye_detector = CascadeClassifier::new("assets/haara/haarcascade_eye.xml")?;
//
//     let mut reduced = Mat::default();
//     let mut faces = Vector::new();
//
//     face_detector.detect_multi_scale(
//         &reduced,
//         &mut faces,
//         1.1,
//         2,
//         CASCADE_SCALE_IMAGE,
//         Size::new(30, 30),
//         Size::new(0, 0),
//     )?;
//
//     println!("faces: {}", faces.len());
//     for face in faces {
//         println!("face {face:?}");
//         let scaled_face = Rect::new(face.x * 4, face.y * 4, face.width * 4, face.height * 4);
//         imgproc::rectangle_def(&mut img, scaled_face, (0, 255, 0).into())?;
//     }
//     highgui::imshow("Test", &img)?;
//
//     highgui::wait_key(0)?;
//
//     Ok(())
// }


use std::thread;
use std::time::Duration;

use opencv::core::{Rect, Size, Vector};
use opencv::prelude::*;
use opencv::{core, highgui, imgproc, videoio, Result};

opencv::opencv_branch_5! {
	use opencv::xobjdetect::{CascadeClassifier, CASCADE_SCALE_IMAGE};
}

opencv::not_opencv_branch_5! {
	use opencv::objdetect::{CascadeClassifier, CASCADE_SCALE_IMAGE};
}

fn main() -> Result<()> {
    const WINDOW: &str = "video capture";
    highgui::named_window_def(WINDOW)?;
    let xml = core::find_file_def("assets/haara/haarcascade_frontalface_default.xml")?;
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?; // 0 is the default camera
    if !cam.is_opened()? {
        panic!("Unable to open default camera!");
    }
    let mut face = CascadeClassifier::new(&xml)?;
    loop {
        let mut frame = Mat::default();
        cam.read(&mut frame)?;
        if frame.size()?.width == 0 {
            thread::sleep(Duration::from_secs(50));
            continue;
        }
        let mut gray = Mat::default();
        imgproc::cvt_color_def(&frame, &mut gray, imgproc::COLOR_BGR2GRAY)?;
        let mut reduced = Mat::default();
        imgproc::resize(&gray, &mut reduced, Size::new(0, 0), 0.25, 0.25, imgproc::INTER_LINEAR)?;
        let mut faces = Vector::new();
        face.detect_multi_scale(
            &reduced,
            &mut faces,
            1.01,
            1,
            CASCADE_SCALE_IMAGE,
            Size::new(5, 5),
            Size::new(0, 0),
        )?;
        println!("faces: {}", faces.len());
        for face in faces {
            println!("face {face:?}");
            let scaled_face = Rect::new(face.x * 4, face.y * 4, face.width * 4, face.height * 4);
            imgproc::rectangle_def(&mut frame, scaled_face, (0, 255, 0).into())?;
        }
        highgui::imshow(WINDOW, &frame)?;
        if highgui::wait_key(10)? > 0 {
            break;
        }
    }
    Ok(())
}