use crate::utils::FrameProcessorFn;
use anyhow::{Context, Result, anyhow};
use gst::prelude::{Cast, ElementExt, GstBinExtManual, ObjectExt, PadExt};
use gst::{Bin, Element, ElementFactory, Pipeline, element_warning};
use gst_app::AppSink;
use std::path::Path;
use std::sync::{Arc, Mutex};
use gst_video::VideoFrame;
use gst_video::video_frame::Readable

pub struct FilePipeline {
    pipeline: Pipeline,
    app_sink: AppSink,
    frame_processor: Option<FrameProcessorFn>,
}

impl FilePipeline {
    pub fn new(file_path: &str, crop_width: i32, crop_height: i32) -> Result<Self> {
        let pipeline = Pipeline::new();

        let source = Self::create_file_source(file_path)?;
        let elements = [
            ("videoconvert", "convert"),
            ("cairooverlay", "cairooverlay"),
            ("videoconvert", "convert_out"),
            ("tee", "tee"),
        ]
            .iter()
            .map(|(factory, name)| ElementFactory::make(factory).name(name).build())
            .collect::<Result<Vec<_>, _>>()?;

        let [convert, cairooverlay, convert_out, tee] = elements.try_into().unwrap();

        pipeline.add_many(&[&source, &convert, &cairooverlay, &convert_out, &tee])?;
        Element::link_many(&[&source, &convert, &cairooverlay, &convert_out, &tee])?;

        //Branch for video processing
    }

    pub fn set_frame_processor(&mut self, f: FrameProcessorFn) {
        self.frame_processor = Some(Arc::new(f));
    }

    fn create_video_process_branch(crop_width: i32, crop_height: i32) -> Result<(Vec<Element>, AppSink)> {
        let queue = gst::ElementFactory::make("queue").name("queue_process").build()?;
        let scale = gst::ElementFactory::make("videoscale").build()?;

        let caps_filter = gst::ElementFactory::make("capsfilter").build()?;
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", gst_video::VideoFormat::Rgb.to_str())
            .field("width", crop_width)
            .field("height", crop_height)
            .build();
        caps_filter.set_property("caps", &caps);

        let app_sink = gst_app::AppSink::builder()
            .drop(true)
            .max_buffers(1)
            .build();
        app_sink.set_property("emit-signals", true);

        Ok((vec![queue, scale, caps_filter, app_sink.upcast()], app_sink))
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

            if !is_video {
                // TODO: add to log
                println!("Ignoring non-video pad");
                return;
            }

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
