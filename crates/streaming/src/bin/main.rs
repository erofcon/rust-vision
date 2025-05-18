use anyhow::{Result, anyhow};
use detection::inference::detection_with_mat;
use detection::model::Model;
use gst::prelude::{
    Cast, ElementExt, GObjectExtManualGst, GstBinExtManual, ObjectExt, PadExt, PadExtManual,
};
use gst::{Buffer, Element, ElementFactory, MessageView, PadProbeData, PadProbeReturn, PadProbeType, Pipeline, element_warning, glib, Caps};
use gst_pbutils::prelude::AudioVisualizerExtManual;
use gst_video::VideoFrameExt;
use image::{DynamicImage, RgbImage};
use ndarray::Array;
use opencv::core::{CV_8UC1, CV_8UC3, CV_8UC4, Mat, Ptr, Scalar, Size, Vector};
use opencv::highgui::imshow;
use opencv::mod_prelude::OpenCVTypeExternContainer;
use opencv::prelude::{BackgroundSubtractorTrait, MatTrait, MatTraitConst};
use opencv::video::{BackgroundSubtractorMOG2, Tracker};
use opencv::{highgui, imgcodecs, imgproc, video};
use std::ffi::c_void;
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

struct FilePipeline {
    pipeline: Pipeline,
    overlay: Element,
    status: Arc<Mutex<String>>,
}

impl FilePipeline {
    pub fn new(
        path: &str,
        buffer_processor: impl Fn(&mut Buffer, i32, i32, Arc<Mutex<String>>, Caps) + Send + Sync + 'static,
    ) -> Result<Self> {
        let status = Arc::new(Mutex::new(String::from("no_motion")));
        let status_for_probe = status.clone();

        let pipeline = Pipeline::new();

        let src = Self::file_src_bin(path)?;

        let video_convert = ElementFactory::make("videoconvert").build()?;

        let scale = ElementFactory::make("videoscale").build()?;
        let videorate = ElementFactory::make("videorate").build()?;

        let fps = 30; // TODO: dynamic get
        let adjusted_fps = fps / 1;

        println!("adjusted fps: {}", adjusted_fps);
        let caps = gst::caps::Caps::builder(glib::gstr!("video/x-raw"))
            .field("format", "RGB")
            .build();
        let caps_filter = ElementFactory::make_with_name("capsfilter", None)?;
        caps_filter.set_property("caps", &caps);

        // let caps_filter = ElementFactory::make("capsfilter").build()?;
        // let caps = gst::Caps::builder("video/x-raw")
        //     .field("format", gst_video::VideoFormat::Rgb.to_str())
        //     // .field("width", 640)
        //     // .field("height", 640)
        //     .field("framerate", gst::Fraction::new(adjusted_fps, 1))
        //     .build();
        // caps_filter.set_property("caps", &caps);

        let video_convert2 = ElementFactory::make("videoconvert").build()?;

        let process_queue = ElementFactory::make_with_name("queue", None)?;

        let process_queue_src = process_queue.static_pad("src").unwrap();

        process_queue_src.add_probe(PadProbeType::BUFFER, move |pad, pad_probe_info| {
            if let Some(PadProbeData::Buffer(buffer)) = &mut pad_probe_info.data {
                let caps = pad.current_caps().expect("Pad has no caps!");
                let s = caps.structure(0).expect("Caps have no structure!");

                let width = s.get::<i32>("width").unwrap();
                let height = s.get::<i32>("height").unwrap();
                let format = s.get::<String>("format").unwrap();

                if format.as_str() != "BGRx" {
                    panic!("Unsupported format")
                };

                buffer_processor(buffer, width, height, status_for_probe.clone(), caps);
            }

            PadProbeReturn::Ok
        });

        let overlay = ElementFactory::make("cairooverlay").build()?;

        let encoder_convert = ElementFactory::make_with_name("videoconvert", None)?;

        let display_sink = ElementFactory::make_with_name("autovideosink", None)?;
        display_sink.set_property_from_str("sync", "false");

        pipeline.add_many(&[
            &src,
            &video_convert,
            &scale,
            &videorate,
            &caps_filter,
            &video_convert2,
            &process_queue,
            &overlay,
            &encoder_convert,
            &display_sink,
        ])?;

        Element::link_many(&[
            &src,
            &video_convert,
            &scale,
            &videorate,
            &caps_filter,
            &video_convert2,
            &process_queue,
            &overlay,
            &encoder_convert,
            &display_sink,
        ])?;

        Ok(Self {
            pipeline,
            overlay,
            status,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.draw();

        self.pipeline.set_state(gst::State::Playing)?;

        let bus = self.pipeline.bus().unwrap();

        for msg in bus.iter_timed(gst::ClockTime::NONE) {
            match msg.view() {
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
                _ => (),
            }
        }

        self.pipeline.set_state(gst::State::Null)?;
        Ok(())
    }

    fn file_src_bin(input_file: &str) -> Result<Element, glib::BoolError> {
        // filesrc -> decodebin -> queue
        let bin = gst::Bin::new();

        let source = ElementFactory::make_with_name("filesrc", None)?;
        source.set_property_from_str("location", input_file);

        let decode_bin = ElementFactory::make_with_name("decodebin", None)?;
        let queue = ElementFactory::make_with_name("queue", None)?;

        bin.add_many([&source, &decode_bin, &queue])?;
        Element::link_many([&source, &decode_bin])?;

        let queue_src = queue.static_pad("src").unwrap();
        let bin_ghost_src_pad = gst::GhostPad::with_target(&queue_src)?;

        bin.add_pad(&bin_ghost_src_pad)?;

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

    fn draw(&self) {
        let status_clone = self.status.clone();
        self.overlay.connect("draw", false, move |args| {
            if let Ok(status) = status_clone.lock() {
                let cr = args[1].get::<&cairo::Context>().unwrap();

                cr.set_source_rgb(0.0, 1.0, 0.0);
                cr.set_font_size(24.0);
                cr.move_to(20.0, 30.0);
                cr.show_text(status.as_str()).expect("Failed to show text");
            }

            None
        });
    }
}

struct Motion {
    mog: Ptr<BackgroundSubtractorMOG2>,
    learning_rate: f64,
    motion_threshold: i32,
}

impl Motion {
    pub fn new() -> Result<Self> {
        let mog = video::create_background_subtractor_mog2(250, 50.0, true)?;

        Ok(Self {
            mog,
            learning_rate: -1.0,
            motion_threshold: 3000,
        })
    }

    pub fn predict(&mut self, frame: Mat) -> Result<bool> {
        let mut motion_mask = Mat::default();

        self.mog
            .apply(&frame, &mut motion_mask, self.learning_rate)?;

        let motion_pixels = opencv::core::count_non_zero(&motion_mask)?;
        let motion_detected = motion_pixels > self.motion_threshold;

        Ok(motion_detected)
    }
}

fn process_buffer(
    buffer: &mut Buffer,
    width: i32,
    height: i32,
    status: Arc<Mutex<String>>,
    tracker: Arc<Mutex<Motion>>,
    model: Arc<Mutex<Model>>,
    caps: Caps,
) {

    // let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
    // let buffer = sample.buffer().ok_or(gst::FlowError::Error)?;
    // let caps = sample.caps().ok_or(gst::FlowError::Error)?;
    let info =
        gst_video::VideoInfo::from_caps(&caps).map_err(|_| gst::FlowError::Error).unwrap();

    let frame = gst_video::VideoFrame::from_buffer_readable(buffer.copy(), &info)
        .map_err(|_| gst::FlowError::Error).unwrap();

    println!("Frame: {:?}", frame.width());

    // let bounding_box_clone = bounding_box.clone();
    // let _ = sender.send((buffer, caps));

    // let map = buffer.map_readable().unwrap();

    // let image = {
    //     let readable = buffer.map_readable().unwrap();
    //     let readable_vec = readable.to_vec();
    //
    //     let image = RgbImage::from_vec(width as u32, height as u32, readable_vec).unwrap();
    //     DynamicImage::ImageRgb8(image)
    // };
    //
    // image.save("buffer.png").unwrap();

    // let image = image.to_rgb8();
    //
    // image.save_with_format("buffer.png", image::ImageFormat::Png).unwrap();
    // let (width, height) = image.dimensions();
    // let ratio = (640 / width).min(640 / height);
    // println!("Processing buffer with ratio: {}", ratio);
    //
    // let (scaled_width, scaled_height) = { (width * ratio, height * ratio) };
    //
    // println!("Scaled width: {}, height: {}", scaled_width, scaled_height);
    // let start_time = Instant::now();
    // // FIXME resize with image crate below is way slower than fast image resize above
    // let scaled_image = image::imageops::resize(
    //     &image,
    //     scaled_width,
    //     scaled_height,
    //     image::imageops::FilterType::Nearest,
    // );
    // let duration = start_time.elapsed();
    //
    // let target_shape = [
    //     1,
    //     3,
    //     640usize,
    //     640usize,
    // ];
    // let mut image_array = Array::zeros(target_shape);
    //
    // for (x, y, rgb) in scaled_image.enumerate_pixels() {
    //     let x = x as usize;
    //     let y = y as usize;
    //     let [r, g, b] = rgb.0;
    //     image_array[[0, 0, y, x]] = (r as f32) / 255.0;
    //     image_array[[0, 1, y, x]] = (g as f32) / 255.0;
    //     image_array[[0, 2, y, x]] = (b as f32) / 255.0;
    // }

    // let scaled_image = image::imageops::resize(
    //     &image,
    //     scaled_dims.width as u32,
    //     scaled_dims.height as u32,
    //     image::imageops::FilterType::Nearest,
    // );

    // let frame_resized = unsafe {
    //     let frame = Mat::new_rows_cols_with_data_unsafe_def(
    //         height,
    //         width,
    //         CV_8UC4,
    //         map.as_ptr() as *mut c_void,
    //     )
    //     .unwrap();
    //
    //     let mut resized_frame = Mat::default();
    //     imgproc::resize(
    //         &frame,
    //         &mut resized_frame,
    //         Size::new(640, 640),
    //         0.0,
    //         0.0,
    //         imgproc::INTER_LINEAR,
    //     )
    //     .unwrap();
    //
    //     resized_frame
    // };
    //
    // if let Ok(model) = model.lock() {
    //     detection_with_mat(model.get_session(), &frame_resized).unwrap();
    // }

    //
    // let size = frame.size();

    // println!("size: {:?}", size);

    // if let Ok(mut t) = tracker.lock() {
    //     if let Ok(predict) = t.predict(frame) {
    //         if let Ok(mut status) = status.lock() {
    //             *status = if predict { "motion" } else { "no_motion" }.to_string();
    //         }
    //     }
    // }

    // let map = buffer.map_readable().unwrap();
    //
    // let mat = unsafe {
    //     Mat::new_rows_cols_with_data_unsafe_def(height, width, CV_8UC4, map.as_ptr() as *mut c_void)
    //         .unwrap()
    // };
    //
    // if let Ok(mut t) = tracker.lock() {
    //     let predict = t.predict(mat).unwrap();
    //
    //     if let Ok(mut status) = status.lock() {
    //         if predict {
    //             *status = "motion".to_string();
    //         } else {
    //             *status = "no_motion".to_string();
    //         }
    //     }
    // }

    // imgcodecs::imwrite("test.jpg", &frame_resized, &Vector::new()).unwrap();
}

fn main() -> Result<()> {
    // unsafe {
    //     std::env::set_var("GST_DEBUG", "3");
    //     std::env::set_var("RUST_BACKTRACE", "full");
    // }

    gst::init()?;

    let path = "D:/Videos/2.mp4";
    let file_path = Path::new(path);
    if !file_path.exists() {
        return Err(anyhow!("File does not exist: {}", file_path.display()));
    };

    let tracker = Arc::new(Mutex::new(Motion::new()?));

    let model = Arc::new(Mutex::new(Model::new("models/yolo11s.onnx")?));

    let mut pipeline = FilePipeline::new(path, move |buff, width, height, status, caps| {
        process_buffer(buff, width, height, status, tracker.clone(), model.clone(), caps);
    })?;

    pipeline.run()?;

    Ok(())
}
