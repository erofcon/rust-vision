use crate::detectors::{Detectors, Inference, Output, Prepare};
use crate::utils::BoundingBox;

use anyhow::{Result, anyhow};

use gst::prelude::*;
use gst_video::video_frame::Readable;
use gst_video::{VideoFormat, VideoFrame, VideoFrameExt, gst};
use motion::motion::Motion;
use opencv::core::{AlgorithmHint, CV_8UC3, CV_8UC4, Mat};
use parking_lot::Mutex;
use rayon::join;
use rayon::prelude::*;
use std::sync::Arc;
use tokio::time::Instant;

type TaskResult = (&'static str, Result<()>);

enum DetectionMode {
    ObjectsDetection,
    MotionDetection,
}

fn video_frame_to_mat(frame: &VideoFrame<Readable>) -> Result<Mat> {
    let width = frame.width() as i32;
    let height = frame.height() as i32;
    let format = frame.format();

    let data = frame
        .plane_data(0)
        .map_err(|e| anyhow!("Failed to get plane data: {:?}", e))?;

    let mat = match format {
        VideoFormat::Bgra => unsafe {
            let mut mat = Mat::new_rows_cols_with_data_unsafe_def(
                height,
                width,
                CV_8UC4,
                data.as_ptr() as *mut _,
            )?;
            mat.clone()
        },
        VideoFormat::Rgb => unsafe {
            let mat = Mat::new_rows_cols_with_data_unsafe_def(
                height,
                width,
                CV_8UC3,
                data.as_ptr() as *mut _,
            )?;

            let mut bgr_mat = Mat::default();
            opencv::imgproc::cvt_color(
                &mat,
                &mut bgr_mat,
                opencv::imgproc::COLOR_RGB2BGR,
                0,
                AlgorithmHint::ALGO_HINT_DEFAULT,
            )?;
            bgr_mat
        },
        VideoFormat::Bgr => unsafe {
            let mut mat = Mat::new_rows_cols_with_data_unsafe_def(
                height,
                width,
                CV_8UC3,
                data.as_ptr() as *mut _,
            )?;
            mat.clone()
        },
        _ => {
            return Err(anyhow!("Unsupported video format: {:?}", format));
        }
    };

    Ok(mat)
}

fn detect_motion(frame: &VideoFrame<Readable>, motion: &mut Motion) -> Result<bool> {
    println!("Detecting motion ...");
    let mat = video_frame_to_mat(frame)?;
    let result = motion.predict(mat)?;
    Ok(result)
}

pub struct DetectionState {
    motion: Arc<Mutex<Motion>>,
    detectors: Detectors,
    mode: DetectionMode,
    frames_without_detection: usize,
    frames_without_motion: usize,
    detection_max_empty_frames: usize,
    motion_max_empty_frames: usize,
}

impl DetectionState {
    pub fn new(
        motion: Arc<Mutex<Motion>>,
        detectors: Detectors,
        detection_max_empty_frames: usize,
        motion_max_empty_frames: usize,
    ) -> Self {
        Self {
            motion,
            detectors,
            mode: DetectionMode::ObjectsDetection,
            frames_without_detection: 0,
            frames_without_motion: 0,
            detection_max_empty_frames,
            motion_max_empty_frames,
        }
    }

    pub fn process_frame(
        &mut self,
        frame: &VideoFrame<Readable>,
        bounding_box: &Arc<Mutex<Vec<(BoundingBox, usize, f32)>>>,
    ) -> Result<bool> {
        match self.mode {
            DetectionMode::ObjectsDetection => {
                let detected = self.detect_objects(frame, bounding_box)?;

                if detected {
                    self.frames_without_detection = 0;
                    Ok(true)
                } else {
                    self.frames_without_detection += 1;
                    if self.frames_without_detection >= self.detection_max_empty_frames {
                        println!("Switching to motion detection mode");
                        self.mode = DetectionMode::MotionDetection;
                        self.frames_without_detection = 0;
                    }
                    Ok(false)
                }
            }
            DetectionMode::MotionDetection => {
                let mut motion = self.motion.lock();
                let motion_detected = detect_motion(frame, &mut *motion)?;

                if motion_detected {
                    println!("Motion detected, switching back to object detection");
                    self.frames_without_motion = 0;
                    self.mode = DetectionMode::ObjectsDetection;

                    drop(motion);
                    return self.detect_objects(frame, bounding_box);
                } else {
                    self.frames_without_motion += 1;
                    if self.frames_without_motion >= self.motion_max_empty_frames {
                        println!("No motion for too long, switching to object detection");
                        self.mode = DetectionMode::ObjectsDetection;
                        self.frames_without_motion = 0;
                    }
                }

                Ok(false)
            }
        }
    }

    fn detect_objects(
        &self,
        frame: &VideoFrame<Readable>,
        bounding_box: &Arc<Mutex<Vec<(BoundingBox, usize, f32)>>>,
    ) -> Result<bool> {
        let start = Instant::now();

        let image = Detectors::convert_gst_image_to_dynamic(frame)?;

        let prepared = Detectors::prepare_dynamic_image_for_yolo11s(&image)
            .map_err(|e| anyhow!("prepare_dynamic_image failed: {:?}", e))?;

        let (persons_res, faces_res): (
            Result<Vec<(BoundingBox, usize, f32)>>,
            Result<Vec<(BoundingBox, usize, f32)>>,
        ) = join(
            || -> Result<Vec<(BoundingBox, usize, f32)>> {
                if let Some(person_model) = &self.detectors.person {
                    // let model_guard = person_model;
                    let session = person_model.get_session();

                    let inf = Detectors::yolo11_inference(session, prepared.clone())?;
                    let persons =
                        Detectors::process_yolo11s_output(inf, 640f32, 640f32, Some(&[0_usize]))?;

                    Ok(persons)
                } else {
                    Ok(Vec::new())
                }
            },
            || -> Result<Vec<(BoundingBox, usize, f32)>> {
                if let Some(face_models) = &self.detectors.face {
                    // let det_guard = face_models.detection.read();
                    let det_session = face_models.detection.get_session();

                    let inf = Detectors::yolo11_inference(det_session, prepared.clone())?;

                    let faces = Detectors::process_yolo11s_output(inf, 640f32, 640f32, None)?;

                    Ok(faces)
                } else {
                    Ok(Vec::new())
                }
            },
        );

        let persons = persons_res?;
        let faces = faces_res?;

        let face_embeddings: Vec<_> = if let Some(face_models) = &self.detectors.face {
            let model_session = face_models.recognition.get_session();
            faces
                .par_iter()
                .filter_map(|(bbox, _class, _prob)| Detectors::crop_face(&image, bbox).ok())
                .map(|crop| {
                    // TODO: get WxH from config
                    let img = Detectors::resize(&crop, 160, 160)?;
                    let prep = Detectors::preprocess_image_for_recognition(&img)
                        .map_err(|e| anyhow!("prep failed: {:?}", e))?;
                    Detectors::extract_face_embedding(model_session, prep)
                })
                .collect()
        } else {
            Vec::new()
        };

        let mut boxes = bounding_box.lock();

        boxes.clear();

        boxes.extend(persons.clone());
        boxes.extend(faces);

        let duration = start.elapsed();

        println!("Time elapsed in detect_objects is: {:?}", duration);

        Ok(!persons.is_empty())
    }
}
