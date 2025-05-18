use anyhow::{Result, anyhow};
use gst::{Buffer, Caps};
use gst_streaming::pipeline::GstPipeline;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

fn process_buffer(buffer: &Buffer, caps: &Caps) {}

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

    let mut pipeline = GstPipeline::new(path, rtmp_url, move |buffer, caps| {
        process_buffer(buffer, caps);
    })?;

    pipeline.run()?;

    Ok(())
}
