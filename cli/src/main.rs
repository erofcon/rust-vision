use anyhow::Result;
use gst_streaming::pipeline::VideoPipeline;
use inference::Inference;
use std::sync::Arc;
use std::time::Duration;
use std::{thread};

#[derive(Debug)]
struct VideoSource {
    url: String,
    rtmp: String,
}

fn main() -> Result<()> {
    std::env::set_var("GST_DEBUG", "3");
    std::env::set_var("RUST_BACKTRACE", "full");

    gst::init()?;

    let model_path = "D:/RustRoverProjects/rust-vision/models/yolo11s.onnx";
    let video_sources = vec![
        VideoSource {
            url: String::from("D:/RustRoverProjects/rust-vision/videos/troy.mp4"),
            rtmp: String::from("rtmp://localhost/live/stream_1"),
        },
        // VideoSource {
        //     url: String::from("D:/RustRoverProjects/rust-vision/videos/troy.mp4"),
        //     rtmp: String::from("rtmp://localhost/live/stream_2"),
        // },
        // VideoSource {
        //     url: String::from("D:/RustRoverProjects/rust-vision/videos/troy.mp4"),
        //     rtmp: String::from("rtmp://localhost/live/stream_3"),
        // },
    ];

    let model_input_width = 640;
    let model_input_height = 640;

    println!("initializing Inference model: {}", model_path);
    let inference = Arc::new(Inference::new(model_path));

    let mut handles = Vec::new();

    for video_source in video_sources {
        thread::sleep(Duration::from_secs(3));
        let inference_clone = inference.clone();
        let source = video_source.url;
        let handle = std::thread::spawn(move || -> Result<()> {
            println!("Run pipeline: {}", source);
            let mut pipeline = VideoPipeline::new(
                &source,
                model_input_width,
                model_input_height,
                video_source.rtmp.as_str(),
            )?;

            let detected_objects_clone = pipeline.detected_objects();
            let file_info = pipeline.file_info();

            pipeline.set_frame_processor(move |frame| {
                let mut bboxes = Vec::new();
                if let Ok(file_info) = file_info.lock() {
                    bboxes = inference_clone.inference(
                        frame,
                        file_info.width as usize,
                        file_info.height as usize,
                    )?;
                }
                if let Ok(mut detected_objects) = detected_objects_clone.lock() {
                    *detected_objects = bboxes;
                }
                Ok(())
            });

            pipeline.start()?;
            Ok(())
        });
        handles.push(handle);
    }

    for handle in handles {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => eprintln!("Error in pipeline: {:?}", e),
            Err(e) => eprintln!("Panic in thread: {:?}", e),
        }
    }

    Ok(())
}
