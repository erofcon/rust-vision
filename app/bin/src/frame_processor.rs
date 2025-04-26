use anyhow::Result;
use gst_video::video_frame::{Readable, VideoFrame};
use std::sync::Arc;

pub type FrameProcessor = dyn Fn(&VideoFrame<Readable>) -> Result<()> + Send + Sync + 'static;

pub fn build_frame_processor() -> Arc<FrameProcessor> {
    Arc::new(move |_| Ok(()))
}
