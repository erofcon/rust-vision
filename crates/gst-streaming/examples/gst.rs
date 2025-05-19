use anyhow::{Result, anyhow};
use common::detection_state::DetectionState;
use detection::model::Model;
use gst_streaming::pipeline::GstPipeline;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use motion::motion::Motion;
use std::path::Path;
use std::sync::{Arc, Mutex};

fn process_buffer(buffer: &VideoFrame<Readable>, state: &Arc<Mutex<DetectionState>>) -> Result<()> {
    if let Ok(mut state_guard) = state.lock() {
        let detection_result = state_guard.process_frame(buffer)?;

        Ok(())
    } else {
        eprintln!("Failed to lock detection state mutex");

        Ok(())
    }
}

fn main() -> Result<()> {
    // unsafe {
    //     std::env::set_var("GST_DEBUG", "3");
    //     std::env::set_var("RUST_BACKTRACE", "full");
    // }

    gst::init()?;

    let path = "D:/Videos/2.mp4";
    let file_path = Path::new(path);
    if !file_path.exists() {
        return Err(anyhow!("File does not exist: {}", file_path.display()));
    };

    let rtmp_url = "rtmp://localhost/live/stream_1";

    let motion = Arc::new(Mutex::new(Motion::new()?));
    let model = Arc::new(Mutex::new(Model::new("models/yolo11s.onnx")?));

    let detection_state = Arc::new(Mutex::new(DetectionState::new(motion, model, 120, 30)));

    let detection_state_clone = detection_state.clone();

    let mut pipeline = GstPipeline::new(path, rtmp_url, move |buffer| {
        process_buffer(buffer, &detection_state_clone).unwrap();
    })?;

    pipeline.run()?;

    Ok(())
}
