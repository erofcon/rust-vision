use crate::model::Model;
use crate::utils::{BoundingBox, Detection};
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use image::DynamicImage;
use ndarray::{Array, Array4, ArrayBase, Dim, Ix, IxDyn, OwnedRepr};
use opencv::core::Mat;
use ort::inputs;
use ort::session::{Session, SessionOutputs};
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

    fn prepare_mat_img(
        img: &Mat,
        target_width: usize,
        target_height: usize,
    ) -> anyhow::Result<ArrayBase<OwnedRepr<f32>, Dim<[Ix; 4]>>>;

    fn resize_mat_with_padding(src: &Mat) -> anyhow::Result<(Mat, f32, f32, f32)>;

    fn crop_detection(input_img: &Mat, detection: &Detection) -> anyhow::Result<Mat>;

    fn align_face(face: &Mat, detection: &Detection) -> anyhow::Result<Mat>;

    fn resize_mat(img: &Mat, target_width: i32, target_height: i32) -> anyhow::Result<Mat>;
    fn preprocess_image_for_recognition_mat(
        image: &Mat,
    ) -> anyhow::Result<Array4<f32>, Box<dyn std::error::Error>>;
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

    fn inference(
        session: &Session,
        model_input: ArrayBase<OwnedRepr<f32>, Dim<[Ix; 4]>>,
    ) -> anyhow::Result<Array<f32, IxDyn>>;
}

pub trait Output {
    fn process_yolo11s_output(
        output: Array<f32, IxDyn>,
        original_img_width: f32,
        original_img_height: f32,
        input_width: f32,
        input_height: f32,
        scale: f32,
        pad_x: f32,
        pad_y: f32,
        prob_threshold: Option<f32>,
        out_classes: Option<&[usize]>,
    ) -> anyhow::Result<Vec<BoundingBox>, Box<dyn std::error::Error>>;

    fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> anyhow::Result<f32>;

    fn process_detections(
        outputs: &Array<f32, IxDyn>,
        original_width: i32,
        original_height: i32,
        scale: f32,
        pad_x: f32,
        pad_y: f32,
    ) -> anyhow::Result<Vec<Detection>>;
}
