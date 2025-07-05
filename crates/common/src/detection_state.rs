use crate::utils::Detectors;
use anyhow::{Result, anyhow};
use detection::inference::{
    extract_face_embedding, prepare_dynamic_image, preprocess_image_for_recognition,
    process_output, yolov11_inference,
};
use detection::utils::{BoundingBox, convert_gst_image_to_dynamic};
use face_recognition::face_recognition::*;
use gst::prelude::*;
use gst_video::video_frame::Readable;
use gst_video::{VideoFormat, VideoFrame, VideoFrameExt, gst};
use motion::motion::Motion;
use opencv::core::{AlgorithmHint, CV_8UC3, CV_8UC4, Mat};
use parking_lot::Mutex;
use rayon::join;
use rayon::prelude::*;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
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

    pub fn process_frame(&mut self, frame: &VideoFrame<Readable>) -> Result<bool> {
        match self.mode {
            DetectionMode::ObjectsDetection => {
                let detected = self.detect_objects(frame)?;

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
                    return self.detect_objects(frame);
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

    fn detect_objects_base(&self, frame: &VideoFrame<Readable>) -> Result<bool> {
        let mut has_detection = false;
        let image = convert_gst_image_to_dynamic(frame)?;

        let prepare = prepare_dynamic_image(&image).unwrap();

        if let Some(person_model) = &self.detectors.person {
            let model = person_model.read();
            let inference = yolov11_inference(model.get_session(), prepare.clone())?;

            let results = process_output(inference, Some(&[0_usize]))?;

            has_detection = !results.is_empty();
        }

        if let Some(face_models) = &self.detectors.face {
            let mut faces = Vec::new();

            {
                let detection_model = face_models.detection.read();
                let inference = yolov11_inference(detection_model.get_session(), prepare)?;

                faces = process_output(inference, None)?;
            }
            {
                if !faces.is_empty() {
                    let recognition_model = face_models.recognition.read();
                    for (bbox, _class_id, _prob) in faces.iter() {
                        let crop = crop_face(&image, bbox)?;
                        let img = resize(&crop, 160, 160)?;
                        let prepare = preprocess_image_for_recognition(&img).unwrap();

                        let _emb =
                            extract_face_embedding(recognition_model.get_session(), prepare)?;
                    }
                }
            }
        }

        Ok(has_detection)
    }

    fn detect_objects(&self, frame: &VideoFrame<Readable>) -> Result<bool> {
        let start = Instant::now();

        let image = convert_gst_image_to_dynamic(frame)?;
        let prepared = prepare_dynamic_image(&image)
            .map_err(|e| anyhow!("prepare_dynamic_image failed: {:?}", e))?;

        let (persons_res, faces_res): (
            Result<Vec<(BoundingBox, usize, f32)>>,
            Result<Vec<(BoundingBox, usize, f32)>>,
        ) = join(
            || -> Result<Vec<(BoundingBox, usize, f32)>> {
                if let Some(person_model) = &self.detectors.person {
                    let model_guard = person_model.read();
                    let session = model_guard.get_session();

                    let inf = yolov11_inference(session, prepared.clone())?;
                    let persons = process_output(inf, Some(&[0_usize]))?;

                    Ok(persons)
                } else {
                    Ok(Vec::new())
                }
            },
            || -> Result<Vec<(BoundingBox, usize, f32)>> {
                if let Some(face_models) = &self.detectors.face {
                    let det_guard = face_models.detection.read();
                    let det_session = det_guard.get_session();

                    let inf = yolov11_inference(det_session, prepared.clone())?;
                    let faces = process_output(inf, None)?;

                    Ok(faces)
                } else {
                    Ok(Vec::new())
                }
            },
        );

        let persons = persons_res?;
        let faces = faces_res?;

        let face_embeddings: Vec<_> = if let Some(face_models) = &self.detectors.face {
            let rec_guard = face_models.recognition.read();
            let rec_session = rec_guard.get_session();

            faces
                .par_iter()
                .filter_map(|(bbox, _class, _prob)| crop_face(&image, bbox).ok())
                .map(|crop| {
                    // TODO: get WxH from config
                    let img = resize(&crop, 160, 160)?;
                    let prep = preprocess_image_for_recognition(&img)
                        .map_err(|e| anyhow!("prep failed: {:?}", e))?;
                    extract_face_embedding(rec_session, prep)
                })
                .collect()
        } else {
            Vec::new()
        };

        let duration = start.elapsed();

        println!("Time elapsed in detect_objects is: {:?}", duration);

        Ok(!persons.is_empty())
    }
}
