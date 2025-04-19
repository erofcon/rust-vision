use std::sync::Arc;
use anyhow::Result;
use gst_video::video_frame::{Readable, VideoFrame};
use gst_video::VideoFrameExt;

pub type FrameProcessor = dyn Fn(&VideoFrame<Readable>) -> Result<()> + Send + Sync + 'static;

pub fn build_frame_processor() -> Arc<FrameProcessor> {
    Arc::new(move |frame| {
        println!("Frame width: {}", frame.width());
        Ok(())
    })
}
