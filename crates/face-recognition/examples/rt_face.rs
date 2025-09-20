use anyhow::Result;
use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider};
use ort::session::Session;

use rust_faces::{
    viz, FaceDetection, FaceDetectorBuilder, ToArray3,
    ToRgb8,
};

pub fn load_model(detection_model_path: &str) -> Result<Session> {
    ort::init()
        .with_execution_providers([
            CUDAExecutionProvider::default().build().error_on_failure(),
            CPUExecutionProvider::default().build().error_on_failure(),
        ])
        .commit()?;

    Ok(Session::builder()?
        .with_inter_threads(4)?
        .commit_from_file(detection_model_path)?)
}

fn main() -> Result<()> {
    let model_path = "models/face-recognition.onnx";

    let model = load_model(model_path)?;

    println!("{:?}", model.inputs);
    println!("{:?}", model.outputs);



    Ok(())



}
