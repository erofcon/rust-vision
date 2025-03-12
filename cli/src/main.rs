use anyhow::Result;
use gst_streaming::pipeline::VideoPipeline;

fn main() -> Result<()> {
    std::env::set_var("GST_DEBUG", "3");
    std::env::set_var("RUST_BACKTRACE", "full");

    let video_source = "D:/RustRoverProjects/rust-vision/video/troy.mp4";

    println!("Initializing video pipeline from: {}", video_source);

    let mut pipeline =
        VideoPipeline::new(&video_source, 640, 640).expect("Could not create video pipeline");

    pipeline.start().expect("Could not start pipeline");

    Ok(())
}
