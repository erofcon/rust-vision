use crate::model::Model;
use crate::utils::BoundingBox;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use image::DynamicImage;
use ndarray::{Array, Array4, IxDyn};
use opencv::core::Mat;
use ort::session::Session;
use std::sync::Arc;

pub struct FaceDetectors {
    pub detection: Arc<Model>,
    pub recognition: Arc<Model>,
}

pub struct Detectors {
    pub person: Option<Arc<Model>>,
    pub face: Option<FaceDetectors>,
}

pub trait Prepare {
    fn prepare_dynamic_image_for_yolo11s(
        image: &DynamicImage,
    ) -> anyhow::Result<Array4<f32>, Box<dyn std::error::Error>>;

    fn preprocess_image_for_recognition(
        image: &DynamicImage,
    ) -> anyhow::Result<Array4<f32>, Box<dyn std::error::Error>>;

    fn crop_face(image: &DynamicImage, bbox: &BoundingBox) -> anyhow::Result<DynamicImage>;

    fn resize(frame: &DynamicImage, width: u32, height: u32) -> anyhow::Result<DynamicImage>;

    fn convert_gst_image_to_dynamic(frame: &VideoFrame<Readable>) -> anyhow::Result<DynamicImage>;

    fn video_frame_to_mat(frame: &VideoFrame<Readable>) -> anyhow::Result<Mat>;
}

pub trait Inference {
    fn yolo11_inference(
        session: &Session,
        frame: Array<f32, ndarray::Dim<[usize; 4]>>,
    ) -> anyhow::Result<Array<f32, IxDyn>>;

    fn extract_face_embedding(
        session: &Session,
        input_tensor: Array4<f32>,
    ) -> anyhow::Result<Vec<f32>>;
}

pub trait Output {
    fn process_yolo11s_output(
        output: Array<f32, IxDyn>,
        original_img_width: f32,
        original_img_height: f32,
        prob_threshold: Option<f32>,
        out_classes: Option<&[usize]>,
    ) -> anyhow::Result<Vec<(BoundingBox, usize, f32)>>;

    fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> anyhow::Result<f32>;
}
