use anyhow::{anyhow, Result};
use gst::prelude::{
    Cast, ElementExt, ElementExtManual, GstBinExtManual, GstObjectExt, ObjectExt, PadExt,
};
use gst::{element_error, element_warning};
use gst_video::VideoFrame;
use std::sync::Arc;

pub type FrameProcessorFn = Arc<
    dyn Fn(&VideoFrame<gst_video::video_frame::Readable>) -> Result<()> + Send + Sync + 'static,
>;

pub struct VideoPipeline {
    pipeline: gst::Pipeline,
    app_sink: gst_app::AppSink,
    overlay: Option<gst::Element>,
    frame_processor: Option<FrameProcessorFn>,
}

impl VideoPipeline {
    pub fn new(path: &str, model_input_width: i32, model_input_height: i32) -> Result<Self> {
        gst::init()?;

        let pipeline = gst::Pipeline::new();

        // Initial elements for reading the stream
        let source = Self::create_source(path)?;
        let convert = gst::ElementFactory::make("videoconvert").build()?;
        let tee = gst::ElementFactory::make("tee").build()?;

        pipeline.add_many(&[&source, &convert, &tee])?;

        // Linking elements before branching
        gst::Element::link_many(&[&source, &convert, &tee])?;

        // Create branch for video processing
        let queue_process = gst::ElementFactory::make("queue").build()?;
        let scale = gst::ElementFactory::make("videoscale").build()?;
        let caps_filter = gst::ElementFactory::make("capsfilter").build()?;
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", gst_video::VideoFormat::Rgb.to_str())
            .field("width", model_input_width)
            .field("height", model_input_height)
            .build();

        caps_filter.set_property("caps", &caps);
        let app_sink = gst_app::AppSink::builder()
            .drop(true)
            .max_buffers(1)
            .build();
        app_sink.set_property("emit-signals", true);

        pipeline.add_many(&[&queue_process, &scale, &caps_filter, app_sink.upcast_ref()])?;
        gst::Element::link_many(&[&queue_process, &scale, &caps_filter, app_sink.upcast_ref()])?;

        let tee_src_pad_template = tee.pad_template("src_%u").expect("Failed to pad template");
        let tee_process_pad = tee
            .request_pad(&tee_src_pad_template, None, None)
            .expect("Failed to request process pad from tee");
        let queue_process_pad = queue_process
            .static_pad("sink")
            .expect("Failed to get queue_process sink pad");
        tee_process_pad.link(&queue_process_pad)?;

        // Create branch for video display

        let queue_display = gst::ElementFactory::make("queue").build()?;

        let convert = gst::ElementFactory::make("videoconvert").build()?;

        let cairooverlay = gst::ElementFactory::make("cairooverlay").build()?;

        let convert_out = gst::ElementFactory::make("videoconvert").build()?;
        let video_sink = gst::ElementFactory::make("autovideosink").build()?;

        pipeline.add_many(&[
            &queue_display,
            &convert,
            &cairooverlay,
            &convert_out,
            &video_sink,
        ])?;

        gst::Element::link_many(&[
            &queue_display,
            &convert,
            &cairooverlay,
            &convert_out,
            &video_sink,
        ])?;

        let tee_display_pad = tee
            .request_pad(&tee_src_pad_template, None, None)
            .expect("Failed to request display pad from tee");
        let queue_display_pad = queue_display
            .static_pad("sink")
            .expect("Failed to get queue_display sink pad");
        tee_display_pad.link(&queue_display_pad)?;

        Ok(VideoPipeline {
            pipeline,
            app_sink,
            overlay: Some(cairooverlay),
            frame_processor: None,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        // Draw
        if let Some(overlay) = &self.overlay {
            overlay.connect("draw", false, move |args| None);
        }

        if self.frame_processor.is_some() {
            let processor = match &self.frame_processor {
                Some(proc) => proc.clone(),
                None => return Err(anyhow!("Frame processor not set")),
            };

            self.app_sink.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |appsink| {
                        let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                        let buffer = sample.buffer().ok_or_else(|| {
                            element_error!(
                                appsink,
                                gst::ResourceError::Failed,
                                ("Failed to get buffer from appsink")
                            );

                            gst::FlowError::Error
                        })?;

                        let caps = match sample.caps() {
                            Some(caps) => caps,
                            None => return Ok(gst::FlowSuccess::Ok),
                        };

                        let info = match gst_video::VideoInfo::from_caps(caps) {
                            Ok(info) => info,
                            Err(_) => return Ok(gst::FlowSuccess::Ok),
                        };

                        let frame =
                            match gst_video::VideoFrame::from_buffer_readable(buffer.copy(), &info)
                            {
                                Ok(frame) => frame,
                                Err(_) => return Ok(gst::FlowSuccess::Ok),
                            };

                        if let Err(err) = processor(&frame) {
                            eprintln!("Error processing frame: {:?}", err);
                        }

                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build(),
            );
        }

        self.pipeline.set_state(gst::State::Playing)?;

        let bus = self.pipeline.bus().expect("Failed to get pipeline bus");
        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            match msg.view() {
                gst::MessageView::Eos(..) => {
                    println!("End of stream reached");
                    break;
                }
                gst::MessageView::Error(err) => {
                    eprintln!(
                        "Error from {:?}: {} ({:?})",
                        err.src().map(|s| s.path_string()),
                        err.error(),
                        err.debug()
                    );
                    break;
                }
                _ => (),
            }
        }

        Ok(())
    }

    pub fn set_frame_processor<F>(&mut self, processor: F)
    where
        F: Fn(&VideoFrame<gst_video::video_frame::Readable>) -> Result<()> + Send + Sync + 'static,
    {
        self.frame_processor = Some(Arc::new(processor));
    }

    fn create_source(path: &str) -> Result<gst::Element> {
        let bin = gst::Bin::new();

        let src = match path {
            s if s.starts_with("rtsp://") => panic!("Unsupported {} video source", s),

            s if std::path::Path::new(s).exists() => gst::ElementFactory::make("filesrc")
                .property("location", s)
                .build()?,
            s => panic!("Unsupported source type {}", s),
        };

        let decode_bin = gst::ElementFactory::make("decodebin")
            .build()
            .expect("error building decodebin");

        let queue = gst::ElementFactory::make_with_name("queue", None)?;

        bin.add_many([&src, &decode_bin, &queue])?;
        gst::Element::link_many([&src, &decode_bin])?;

        let queue_src = queue.static_pad("src").unwrap();
        let bin_ghost_src_pad = gst::GhostPad::with_target(&queue_src)?;

        bin.add_pad(&bin_ghost_src_pad)?;

        let queue_weak = queue.downgrade();
        decode_bin.connect_pad_added(move |d_bin, src_pad| {
            if let Some(queue) = queue_weak.upgrade() {
                let (_, is_video) = {
                    let media_type = src_pad.current_caps().and_then(|caps| {
                        caps.structure(0).map(|s| {
                            let name = s.name();
                            (name.starts_with("audio/"), name.starts_with("video/"))
                        })
                    });

                    match media_type {
                        None => {
                            element_warning!(
                                d_bin,
                                gst::CoreError::Negotiation,
                                ("Failed to get media type from pad {}", src_pad.name())
                            );

                            return;
                        }
                        Some(media_type) => media_type,
                    }
                };

                if !is_video {
                    println!("Ignoring non-video pad");
                    return;
                }


                let sink_pad = queue
                    .static_pad("sink")
                    .expect("The queue element must have a sink-pad");

                if let Err(err) = src_pad.link(&sink_pad) {
                    eprintln!(
                        "Failed to link decodebin src pad to queue sink pad: {:?}",
                        err
                    );
                } else {
                    println!("Successfully linked decodebin src pad to queue sink pad");
                }
            } else {
                eprintln!("Late linking: source_bin queue element has been dropped");
            }
        });

        Ok(bin.upcast())
    }
}
