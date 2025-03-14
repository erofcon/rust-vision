use anyhow::Result;
use gst_streaming::pipeline::VideoPipeline;
use inference::Inference;

fn main() -> Result<()> {
    std::env::set_var("GST_DEBUG", "3");
    std::env::set_var("RUST_BACKTRACE", "full");

    // let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models/yolo11s.onnx");
    let model = "D:/RustRoverProjects/rust-vision/models/yolo11s.onnx";
    let video_source = "D:/RustRoverProjects/rust-vision/video/troy.mp4";

    let model_input_width = 640;
    let model_input_height = 640;

    println!("Initializing video pipeline from: {}", video_source);

    let session = Inference::new(&model);

    let mut pipeline = VideoPipeline::new(&video_source, model_input_width, model_input_height)
        .expect("Could not create video pipeline");

    pipeline.set_frame_processor(move |frame| {
        session.inference(
            frame,
            model_input_width as usize,
            model_input_height as usize,
        );

        Ok(())
    });

    pipeline.start().expect("Could not start pipeline");

    Ok(())
}
