use crate::utils::FrameProcessorFn;
use anyhow::{Context, Result, anyhow};
use gst::prelude::{Cast, ElementExt, GstBinExtManual, ObjectExt, PadExt};
use gst::{Bin, Element, ElementFactory, Pipeline, element_warning};
use gst_app::AppSink;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct FilePipeline {
    pipeline: Pipeline,
    app_sink: AppSink,
    frame_processor: Option<FrameProcessorFn>,
}

impl FilePipeline {
    pub fn new(file_path: &str, crop_width: u16, crop_height: u16) -> Result<Self> {
        let pipeline = Pipeline::new();
    }

    fn create_file_source(file_path: &str) -> Result<Element> {
        let bin = Bin::new();

        let file_path = Path::new(&file_path);
        if !file_path.exists() {
            return Err(anyhow!("File does not exist: {}", file_path));
        };

        let src = ElementFactory::make("filesrc")
            .property("location", file_path)
            .build()?;

        let decode_bin = ElementFactory::make("decodebin").build()?;

        let queue = ElementFactory::make("queue").build()?;

        bin.add_many([&src, &decode_bin, &queue])?;
        Element::link_many([&src, &decode_bin])?;

        let queue_src = queue.static_pad("src").unwrap();
        let ghost_pad = gst::GhostPad::with_target(&queue_src)?;
        bin.add_pad(&ghost_pad)?;

        let queue_weak = queue.downgrade();
        decode_bin.connect_pad_added(move |d_bin, src_pad| {
            let queue = match queue_weak.upgrade() {
                Some(q) => q,
                None => {
                    return;
                }
            };

            let is_video = src_pad
                .current_caps()
                .and_then(|caps| caps.structure(0).map(|s| s.name().starts_with("video/")))
                .unwrap_or(false);

            let sink_pad = queue
                .static_pad("sink")
                .expect("Queue must have a sink pad");
            if let Err(err) = src_pad.link(&sink_pad) {
                // TODO: add to log

                element_warning!(
                    d_bin,
                    gst::CoreError::Negotiation,
                    ("Failed to link decodebin pad to queue: {:?}", err)
                );
            } else {
                println!("Successfully linked video pad to queue");
            }
        });

        Ok(bin.upcast())
    }
}
