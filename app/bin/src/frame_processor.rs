use anyhow::Result;
use detection::inference::run;
use detection::model::Model;
use detection::utils::BoundingBox;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use std::sync::{Arc, Mutex};
use streaming::utils::FrameProcessor;
use std::thread::sleep;
use std::time::Duration;


pub fn build_frame_processor(
    model: Arc<Model>,
    model_input_width: i32,
    model_input_height: i32,
) -> Arc<FrameProcessor> {
    let model = model.clone();

    Arc::new(
        move |frame: &VideoFrame<Readable>,
              bounding_box: Arc<Mutex<Vec<(BoundingBox, usize, f32)>>>,
              original_w: &i32,
              original_h: &i32|
              -> Result<()> {

            sleep(Duration::from_millis(100));

            let result = run(
                model.get_session(),
                frame,
                *original_w,
                *original_h,
                model_input_width,
                model_input_height,
                Some(&[0]),
            )?;

            if let Ok(mut boxes) = bounding_box.lock() {
                *boxes = result;
            }

            Ok(())
        },
    )
}
