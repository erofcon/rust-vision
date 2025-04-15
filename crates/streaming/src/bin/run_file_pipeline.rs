// TODO: removed in production
use anyhow::Result;
use streaming::file_pipeline::FilePipeline;

fn main() -> Result<()> {
    gst::init()?;
    let file_path = r"D:\RustRoverProjects\rust-vision\test_video\troy.mp4";

    let mut pipeline = FilePipeline::new(file_path, 640, 640, "rtmp://localhost/live/stream_1")?;

    pipeline.set_frame_processor(move |_| Ok(()));
    pipeline.start()?;

    Ok(())
}
