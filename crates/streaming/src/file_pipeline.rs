use anyhow::{Context, Result, anyhow};
use gst::prelude::{
    Cast, ElementExt, ElementExtManual, GObjectExtManualGst, GstBinExtManual, GstObjectExt,
    ObjectExt, PadExt,
};
use gst::{Bin, Element, ElementFactory, Pipeline, element_warning};
use gst_app::{AppSink, AppSinkCallbacks};
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct FilePipeline {
    pipeline: Pipeline,
    app_sink: AppSink,
    frame_processor:
        Option<Arc<dyn Fn(&VideoFrame<Readable>) -> Result<()> + Send + Sync + 'static>>,
    cancel_flag: Arc<AtomicBool>,
}

impl FilePipeline {
    pub fn new(
        file_path: &str,
        scale_width: i32,
        scale_height: i32,
        rtmp_url: &str,
    ) -> Result<Self> {
        let pipeline = Pipeline::new();

        let source = Self::create_file_source(file_path)?;
        let elements = [
            ("videoconvert", "convert"),
            ("cairooverlay", "cairooverlay"),
            ("videoconvert", "convert_out"),
            ("tee", "tee"),
        ]
        .iter()
        .map(|(factory, name)| ElementFactory::make(factory).name(*name).build())
        .collect::<Result<Vec<_>, _>>()?;

        let [convert, cairooverlay, convert_out, tee] = elements.try_into().unwrap();

        pipeline.add_many(&[&source, &convert, &cairooverlay, &convert_out, &tee])?;
        Element::link_many(&[&source, &convert, &cairooverlay, &convert_out, &tee])?;

        // Branch for video processing
        let (process_branch, app_sink) =
            Self::create_video_process_branch(scale_width, scale_height)?;
        pipeline.add_many(&process_branch)?;
        Element::link_many(&process_branch)?;
        Self::connect_tee_branch(&tee, &process_branch[0], "process")?;

        // Branch for displaying video
        // TODO: removed in production
        let display_branch = Self::create_display_branch()?;
        pipeline.add_many(&display_branch)?;
        Element::link_many(&display_branch)?;
        Self::connect_tee_branch(&tee, &display_branch[0], "display")?;

        // RTMP branch
        let rtmp_branch = Self::create_rtmp_branch(rtmp_url)?;
        pipeline.add_many(&rtmp_branch)?;
        Element::link_many(&rtmp_branch)?;
        Self::connect_tee_branch(&tee, &rtmp_branch[0], "rtmp")?;

        Ok(FilePipeline {
            pipeline,
            app_sink,
            frame_processor: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn set_frame_processor<F>(&mut self, f: F)
    where
        F: Fn(&VideoFrame<Readable>) -> Result<()> + Send + Sync + 'static,
    {
        self.frame_processor = Some(Arc::new(f));
    }

    fn create_video_process_branch(
        scale_width: i32,
        scale_height: i32,
    ) -> Result<(Vec<Element>, AppSink)> {
        let queue = ElementFactory::make("queue")
            .name("queue_process")
            .build()?;
        let scale = ElementFactory::make("videoscale").build()?;

        let caps_filter = gst::ElementFactory::make("capsfilter").build()?;
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", gst_video::VideoFormat::Rgb.to_str())
            .field("width", scale_width)
            .field("height", scale_height)
            .build();
        caps_filter.set_property("caps", &caps);

        let app_sink = AppSink::builder().drop(true).max_buffers(1).build();
        app_sink.set_property("emit-signals", true);

        // Clone app_sink before upcast to avoid the ownership issue
        let app_sink_element = app_sink.clone().upcast();
        Ok((vec![queue, scale, caps_filter, app_sink_element], app_sink))
    }

    fn create_display_branch() -> Result<Vec<Element>> {
        // TODO: removed in production
        Ok(vec![
            ElementFactory::make("queue")
                .name("queue_display")
                .build()?,
            ElementFactory::make("videoconvert").build()?,
            ElementFactory::make("autovideosink").build()?,
        ])
    }

    fn create_rtmp_branch(rtmp_url: &str) -> Result<Vec<gst::Element>> {
        let queue = ElementFactory::make("queue").name("queue_rtmp").build()?;
        queue.set_property("max-size-buffers", 1000u32);

        let convert = ElementFactory::make("videoconvert").build()?;

        let x264enc = ElementFactory::make("x264enc").build()?;
        x264enc.set_property_from_str("tune", "zerolatency");
        x264enc.set_property_from_str("speed-preset", "veryfast");
        x264enc.set_property("bitrate", 2000u32);

        let h264parse = ElementFactory::make("h264parse").build()?;

        let flvmux = ElementFactory::make("flvmux").build()?;
        flvmux.set_property("streamable", true);

        let rtmpsink = ElementFactory::make("rtmpsink").build()?;
        rtmpsink.set_property("location", rtmp_url);

        Ok(vec![queue, convert, x264enc, h264parse, flvmux, rtmpsink])
    }

    fn create_file_source(file_path: &str) -> Result<Element> {
        let bin = Bin::new();

        let file_path = Path::new(&file_path);
        if !file_path.exists() {
            return Err(anyhow!("File does not exist: {}", file_path.display()));
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
                // TODO: add to log
                println!("Successfully linked video pad to queue");
            }
        });

        Ok(bin.upcast())
    }

    fn connect_tee_branch(tee: &Element, target: &Element, name: &str) -> Result<()> {
        let tee_template = tee
            .pad_template("src_%u")
            .expect("Failed to get pad template");
        let tee_pad = tee
            .request_pad(&tee_template, None, None)
            .unwrap_or_else(|| panic!("Failed to request {} pad from tee", name));

        let target_pad = target
            .static_pad("sink")
            .unwrap_or_else(|| panic!("Failed to get {} sink pad", name));

        tee_pad.link(&target_pad)?;
        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        // Set up frame handler
        let processor = self
            .frame_processor
            .clone()
            .ok_or_else(|| anyhow!("Frame processor not set"))?;

        self.app_sink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    // Receiving a frame for processing
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
                    let caps = sample.caps().ok_or(gst::FlowError::Error)?;

                    // Creating and processing a video frame
                    let info =
                        gst_video::VideoInfo::from_caps(caps).map_err(|_| gst::FlowError::Error)?;

                    let frame = gst_video::VideoFrame::from_buffer_readable(buffer.copy(), &info)
                        .map_err(|_| gst::FlowError::Error)?;

                    if let Err(err) = processor(&frame) {
                        // TODO: add to log
                        eprintln!("Error processing frame: {:?}", err);
                    }

                    Ok(gst::FlowSuccess::Ok)
                })
                .build(),
        );

        self.pipeline.set_state(gst::State::Playing)?;

        // Processing messages from the bus
        let bus = self.pipeline.bus().expect("Failed to get pipeline bus");
        let cancel_flag = Arc::clone(&self.cancel_flag);

        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            // TODO: add logging and proper shutdown
            if cancel_flag.load(Ordering::SeqCst) {
                println!("Pipeline cancelled");
                self.pipeline
                    .set_state(gst::State::Null)
                    .expect("Failed to set pipeline to NULL state");
                break;
            }

            match msg.view() {
                gst::MessageView::Eos(..) => {
                    println!("End of stream reached");
                    self.pipeline
                        .set_state(gst::State::Null)
                        .expect("Failed to set pipeline to NULL state");
                    break;
                }
                gst::MessageView::Error(err) => {
                    let src_name = err
                        .src()
                        .map(|s| s.name())
                        .unwrap_or_else(|| "unknown".into());
                    println!("Error from {}: {}", src_name, err);
                    break;
                }
                _ => (),
            }
        }

        self.pipeline
            .set_state(gst::State::Null)
            .expect("Failed to set pipeline to NULL state");

        Ok(())
    }

    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.pipeline
            .set_state(gst::State::Null)
            .expect("Failed to set pipeline to NULL state");
    }

    pub fn get_cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel_flag)
    }
}
