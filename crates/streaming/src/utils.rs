use anyhow::Result;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use std::sync::Arc;

pub type FrameProcessorFn =
    Arc<dyn Fn(&VideoFrame<Readable>) -> Result<()> + Send + Sync + 'static>;
