use anyhow::Result;
use opencv::prelude::{BackgroundSubtractorTrait, VideoCaptureTrait, VideoCaptureTraitConst};
use opencv::{
    core::{self, Mat, Point, Scalar},
    highgui, imgproc, video, videoio,
};

fn main() -> Result<()> {
    const MAX_FRAMES: i32 = 10000;
    const LEARNING_RATE: f64 = -1.0;
    const MOTION_THRESHOLD: i32 = 3000;
    const TEXT_COLOR: Scalar = Scalar::new(0.0, 0.0, 255.0, 0.0); // Красный
    const FONT_SCALE: f32 = 1.0;
    const THICKNESS: i32 = 2;

    let video_path = r"D:/Videos/2.mp4";

    let mut mog = video::create_background_subtractor_mog2(250, 50.0, true)?;

    let mut cap = videoio::VideoCapture::from_file(video_path, 0)?;
    if !cap.is_opened()? {
        eprintln!("Не удалось открыть видеозахват");
        return Ok(());
    }

    highgui::named_window("Motion Detection", highgui::WINDOW_AUTOSIZE)?;
    highgui::named_window("Motion Mask", highgui::WINDOW_AUTOSIZE)?;

    for _ in 0..MAX_FRAMES {
        let mut frame = Mat::default();
        if !cap.read(&mut frame)? {
            break;
        }

        let mut resized_frame = Mat::default();
        imgproc::resize(
            &frame,
            &mut resized_frame,
            core::Size::new(640, 640),
            0.0,
            0.0,
            imgproc::INTER_LINEAR,
        )?;

        let mut motion_mask = Mat::default();
        mog.apply(&resized_frame, &mut motion_mask, LEARNING_RATE)?;

        let motion_pixels = core::count_non_zero(&motion_mask)?;
        let motion_detected = motion_pixels > MOTION_THRESHOLD;
        println!("Motion detected: {}", motion_pixels);
        let text = if motion_detected {
            "Motion Detected"
        } else {
            "No Motion"
        };
        let text_pos = Point::new(20, 50);
        imgproc::put_text(
            &mut resized_frame,
            text,
            text_pos,
            imgproc::FONT_HERSHEY_SIMPLEX,
            FONT_SCALE as f64,
            TEXT_COLOR,
            THICKNESS,
            imgproc::LINE_AA,
            false,
        )?;

        highgui::imshow("Motion Detection", &resized_frame)?;
        highgui::imshow("Motion Mask", &motion_mask)?;

        if highgui::wait_key(1)? == 'q' as i32 {
            break;
        }
    }

    cap.release()?;
    highgui::destroy_all_windows()?;

    Ok(())
}

/*
let fps = 30; // базовая частота кадров
       let adjusted_fps = (fps as f64 * 2.0) as i32;
       let rate_caps = gst::Caps::builder("video/x-raw")
           .field("framerate", gst::Fraction::new(adjusted_fps, 1))
           .build();

       let rate_filter = ElementFactory::make_with_name("capsfilter", Some("speed-capsfilter"))?;
       rate_filter.set_property("caps", &rate_caps);
*/
