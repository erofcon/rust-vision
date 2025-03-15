use anyhow::Result;
use gst_streaming::pipeline::VideoPipeline;
use inference::Inference;


fn main() -> Result<()> {
    std::env::set_var("GST_DEBUG", "3");
    std::env::set_var("RUST_BACKTRACE", "full");

    // let model = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models/yolo11s.onnx");
    let model = "D:/RustRoverProjects/rust-vision/models/yolo11s.onnx";
    let video_source = "D:/RustRoverProjects/rust-vision/videos/person.mp4";

    let model_input_width = 640;
    let model_input_height = 640;


    println!("Initializing video pipeline from: {}", video_source);

    let session = Inference::new(&model);

    let mut pipeline = VideoPipeline::new(&video_source, model_input_width, model_input_height)
        .expect("Could not create video pipeline");

    let detected_objects_clone = pipeline.detected_objects();
    let file_info = pipeline.file_info();

    pipeline.set_frame_processor(move |frame| {
        let mut bboxes = Vec::new();

        if let Ok(file_info) = file_info.lock() {
            bboxes =
                session.inference(frame, file_info.width as usize, file_info.height as usize)?;
        }

        if let Ok(mut detected_objects) = detected_objects_clone.lock() {
            *detected_objects = bboxes;
        }

        Ok(())
    });

    pipeline.start().expect("Could not start pipeline");

    Ok(())
}
