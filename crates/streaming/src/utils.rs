use anyhow::Result;
use detection::utils::BoundingBox;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use std::sync::{Arc, Mutex};

pub type FrameProcessor = dyn Fn(&VideoFrame<Readable>, Arc<Mutex<Vec<(BoundingBox, usize, f32)>>>, &i32, &i32) -> Result<()>
    + Send
    + Sync
    + 'static;
