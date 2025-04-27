use anyhow::Result;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;

pub type FrameProcessor =
    dyn Fn(&VideoFrame<Readable>, &i32, &i32) -> Result<()> + Send + Sync + 'static;
