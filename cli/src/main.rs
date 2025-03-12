use anyhow::Result;
use gst_streaming::pipeline::VideoPipeline;
use inference::Inference;

fn main() -> Result<()> {
    std::env::set_var("GST_DEBUG", "3");
    std::env::set_var("RUST_BACKTRACE", "full");

    // let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models/yolo11s.onnx");
    let model = "D:/RustRoverProjects/rust-vision/models/yolo11s.onnx";
    let video_source = "D:/RustRoverProjects/rust-vision/video/troy.mp4";

    println!("Initializing video pipeline from: {}", video_source);

    let session = Inference::new(&model);

    let mut pipeline =
        VideoPipeline::new(&video_source, 640, 640).expect("Could not create video pipeline");

    pipeline.start().expect("Could not start pipeline");

    pipeline_inference_handler(&session, &pipeline)?;

    Ok(())
}

fn pipeline_inference_handler(session: &Inference, pipeline: &VideoPipeline) -> Result<()> {
    Ok(())
}
