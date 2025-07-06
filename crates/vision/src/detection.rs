use crate::model::Model;
use crate::utils::BoundingBox;
use anyhow::Result;
use image::DynamicImage;
use ndarray::{Array, Array4, IxDyn};
use ort::session::Session;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct FaceModels {
    pub detection: Arc<RwLock<Model>>,
    pub recognition: Arc<RwLock<Model>>,
}

impl FaceModels {
    pub fn load_from_file(
        detection_model_path: &str,
        recognition_model_path: &str,
    ) -> Result<FaceModels> {
        let face_detection_model = Arc::new(RwLock::new(Model::new(detection_model_path)?));
        let face_recognition_model = Arc::new(RwLock::new(Model::new(recognition_model_path)?));

        Ok(Self {
            detection: face_detection_model,
            recognition: face_recognition_model,
        })
    }
}

pub struct Detectors {
    pub person: Option<Arc<RwLock<Model>>>,
    pub face: Option<FaceModels>,
}

pub trait Prepare {
    fn prepare_dynamic_image_for_yolo11s(
        image: &DynamicImage,
    ) -> Result<Array4<f32>, Box<dyn std::error::Error>>;

    fn preprocess_image_for_recognition(
        image: &DynamicImage,
    ) -> Result<Array4<f32>, Box<dyn std::error::Error>>;

    fn crop_face(image: &DynamicImage, bbox: &BoundingBox) -> Result<DynamicImage>;

    fn resize(frame: &DynamicImage, width: u32, height: u32) -> Result<DynamicImage>;
}

pub trait Inference {
    fn yolo11_inference(
        session: &Session,
        frame: Array<f32, ndarray::Dim<[usize; 4]>>,
    ) -> Result<Array<f32, IxDyn>>;
}

pub trait Output {
    fn process_yolo11s_output(
        output: Array<f32, IxDyn>,
        original_img_width: f32,
        original_img_height: f32,
        out_classes: Option<&[usize]>,
    ) -> Result<Vec<(BoundingBox, usize, f32)>>;

    fn extract_face_embedding(session: &Session, input_tensor: Array4<f32>) -> Result<Vec<f32>>;
}
