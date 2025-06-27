use crate::discover;
use anyhow::{Result, anyhow};
use gst::prelude::{
    Cast, ElementExt, GObjectExtManualGst, GstBinExtManual, ObjectExt, PadExt, PadExtManual,
};
use gst::prelude::{ElementExtManual, GstBinExt};
use std::any::type_name_of_val;

use gst::{
    Bin, Buffer, Caps, Element, ElementFactory, MessageView, PadProbeData, PadProbeReturn,
    PadProbeType, Pipeline, SeekFlags, SeekType, element_warning, glib,
};
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct GstPipeline {
    pipeline: Pipeline,
    should_stop: Arc<AtomicBool>,
}

impl GstPipeline {
    pub fn new(
        file_path: &str,
        rtmp_url: &str,
        buffer_processor: impl Fn(&VideoFrame<Readable>) + Send + Sync + 'static,
    ) -> Result<Self> {
        let pipeline = Pipeline::new();

        // let file_info = discover::discover(&file_path)?;

        let source = Self::create_file_source(file_path)?;

        let video_convert = ElementFactory::make("videoconvert").build()?;
        let process_videoscale = ElementFactory::make("videoscale").build()?;

        /*
        let fps = 30; // базовая частота кадров
               let adjusted_fps = (fps as f64 * 2.0) as i32;
               let rate_caps = gst::Caps::builder("video/x-raw")
                   .field("framerate", gst::Fraction::new(adjusted_fps, 1))
                   .build();

               let rate_filter = ElementFactory::make_with_name("capsfilter", Some("speed-capsfilter"))?;
               rate_filter.set_property("caps", &rate_caps);
        */

        let caps = Caps::builder(glib::gstr!("video/x-raw"))
            .field("format", gst_video::VideoFormat::Bgr.to_str())
            .field("width", 640)
            .field("height", 640)
            .build();
        let caps_filter = ElementFactory::make_with_name("capsfilter", None)?;
        caps_filter.set_property("caps", &caps);

        let process_queue = ElementFactory::make("queue").build()?;
        process_queue.set_property("max-size-buffers", 3u32);
        process_queue.set_property_from_str("leaky", "no");

        let process_element = ElementFactory::make("identity").build()?;

        let process_element_sink = process_element.static_pad("sink").unwrap();
        process_element_sink.add_probe(PadProbeType::BUFFER, move |pad, pad_probe_info| {
            if let Some(PadProbeData::Buffer(buffer)) = &mut pad_probe_info.data {
                let caps = &pad.current_caps().expect("Pad has no caps!");

                let info = gst_video::VideoInfo::from_caps(caps)
                    .map_err(|_| gst::FlowError::Error)
                    .unwrap();

                let frame = gst_video::VideoFrame::from_buffer_readable(buffer.copy(), &info)
                    .map_err(|_| gst::FlowError::Error)
                    .unwrap();

                buffer_processor(&frame);
            }

            PadProbeReturn::Ok
        });

        let process_out_video_convert = ElementFactory::make("videoconvert").build()?;
        let overlay = ElementFactory::make("cairooverlay").build()?;

        let tee = ElementFactory::make("tee").build()?;

        pipeline.add_many(&[
            &source,
            &video_convert,
            &process_videoscale,
            &caps_filter,
            &process_queue,
            &process_element,
            &process_out_video_convert,
            &overlay,
            &tee,
        ])?;

        Element::link_many(&[
            &source,
            &video_convert,
            &process_videoscale,
            &caps_filter,
            &process_queue,
            &process_element,
            &process_out_video_convert,
            &overlay,
            &tee,
        ])?;

        let display_branch = Self::create_display_branch()?;
        pipeline.add_many(&display_branch)?;
        Element::link_many(&display_branch)?;
        Self::connect_tee_branch(&tee, &display_branch[0], "display")?;

        let rtmp_branch = Self::create_rtmp_branch(rtmp_url)?;
        pipeline.add_many(&rtmp_branch)?;
        Element::link_many(&rtmp_branch)?;
        Self::connect_tee_branch(&tee, &rtmp_branch[0], "rtmp")?;

        Ok(GstPipeline {
            pipeline,
            should_stop: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        let should_stop = self.should_stop.clone();

        self.pipeline.set_state(gst::State::Playing)?;

        let bus = self.pipeline.bus().unwrap();

        loop {
            if should_stop.load(Ordering::SeqCst) {
                println!("Request to stop pipeline detected");
                break;
            }

            // TODO: add logging and proper shutdown

            match bus.timed_pop(gst::ClockTime::from_mseconds(50)) {
                Some(msg) => match msg.view() {
                    MessageView::Eos(..) => {
                        println!("EOS message received, finishing processing");
                        break;
                    }
                    MessageView::Error(err) => {
                        let error = err.error();
                        let debug = err.debug();
                        println!("Error: {}, debug: {:?}", error, debug);
                        return Err(anyhow!("GStreamer error: {}", error));
                    }
                    MessageView::StateChanged(state) => {
                        if state.src() == Some(self.pipeline.upcast_ref::<gst::Object>()) {
                            let old = state.old();
                            let new = state.current();
                            println!("Pipeline has changed its state: {:?} -> {:?}", old, new);
                        }
                    }
                    _ => {}
                },
                None => {
                    // Timeout, continue the cycle
                }
            }
        }

        // for msg in bus.iter_timed(gst::ClockTime::NONE) {
        //     match msg.view() {
        //         MessageView::Eos(..) => {
        //             println!("EOS message received, finishing processing");
        //             break;
        //         }
        //         MessageView::Error(err) => {
        //             let error = err.error();
        //             let debug = err.debug();
        //             println!("Error: {}, debug: {:?}", error, debug);
        //             return Err(anyhow!("GStreamer error: {}", error));
        //         }
        //         MessageView::StateChanged(state) => {
        //             if state.src() == Some(self.pipeline.upcast_ref::<gst::Object>()) {
        //                 let old = state.old();
        //                 let new = state.current();
        //                 println!("Pipeline has changed its state: {:?} -> {:?}", old, new);
        //             }
        //         }
        //         _ => {}
        //         _ => (),
        //     }
        // }

        self.pipeline.set_state(gst::State::Null)?;
        Ok(())
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

        src.set_property("blocksize", 4096u32 * 8);

        let decode_bin = ElementFactory::make("decodebin").build()?;
        decode_bin.set_property("use-buffering", true);

        let queue = ElementFactory::make("queue").build()?;
        queue.set_property("max-size-buffers", 10u32);
        queue.set_property("max-size-bytes", 0u32);
        queue.set_property("max-size-time", 0u64);

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
                println!("Ignoring non-video pad");
                return;
            }

            let sink_pad = queue
                .static_pad("sink")
                .expect("Queue must have a sink pad");
            if let Err(err) = src_pad.link(&sink_pad) {
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

    fn create_display_branch() -> Result<Vec<Element>> {
        let queue = ElementFactory::make("queue").build()?;
        queue.set_property("max-size-buffers", 3u32);

        let convert = ElementFactory::make("videoconvert").build()?;

        let videosink = ElementFactory::make("autovideosink").build()?;

        videosink.set_property("sync", false);

        Ok(vec![queue, convert, videosink])
    }

    fn create_rtmp_branch(rtmp_url: &str) -> Result<Vec<gst::Element>> {
        let queue = ElementFactory::make("queue").name("queue_rtmp").build()?;
        queue.set_property("max-size-buffers", 30u32);
        queue.set_property("max-size-time", 2_000_000_000u64);

        let convert = ElementFactory::make("videoconvert").build()?;

        let x264enc = match ElementFactory::make("nvh264enc").build() {
            Ok(enc) => {
                println!("Using hardware-accelerated NVIDIA encoder");
                enc.set_property_from_str("preset", "low-latency-hq");
                enc.set_property("zerolatency", true);
                enc
            }
            Err(_) => match ElementFactory::make("vaapih264enc").build() {
                Ok(enc) => {
                    println!("Using hardware-accelerated VAAPI encoder");
                    enc.set_property_from_str("rate-control", "cbr");
                    enc.set_property("bitrate", 1000u32);
                    enc
                }
                Err(_) => {
                    println!("Using software x264 encoder");
                    let enc = ElementFactory::make("x264enc").build()?;
                    enc.set_property_from_str("tune", "zerolatency");
                    enc.set_property_from_str("speed-preset", "ultrafast");
                    enc.set_property("bitrate", 1000u32);
                    enc.set_property("key-int-max", 15i32);
                    enc
                }
            },
        };

        let h264parse = ElementFactory::make("h264parse").build()?;

        let flvmux = ElementFactory::make("flvmux").build()?;
        flvmux.set_property("streamable", true);

        let rtmpsink = ElementFactory::make("rtmpsink").build()?;
        rtmpsink.set_property("location", rtmp_url);
        rtmpsink.set_property("sync", false);

        Ok(vec![queue, convert, x264enc, h264parse, flvmux, rtmpsink])
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

    pub fn get_stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.should_stop)
    }

    pub fn stop(&mut self, timeout_ms: u64) -> Result<()> {
        self.should_stop.store(true, Ordering::SeqCst);

        if !self.pipeline.send_event(gst::event::Eos::new()) {
            println!("Failed to send EOS event");
        }

        let start = std::time::Instant::now();
        while start.elapsed().as_millis() < timeout_ms as u128 {
            let state = self.pipeline.current_state();
            if state == gst::State::Null || state == gst::State::Ready {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        println!("Stopping pipeline");
        self.pipeline.set_state(gst::State::Null)?;

        Ok(())
    }
}
