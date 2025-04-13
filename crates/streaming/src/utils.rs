use anyhow::Result;
use gst_video::VideoFrame;
use std::sync::Arc;

pub type FrameProcessorFn = Arc<
    dyn Fn(&VideoFrame<gst_video::video_frame::Readable>) -> Result<()> + Send + Sync + 'static,
>;
