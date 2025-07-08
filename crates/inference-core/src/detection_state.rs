use crate::analyze::{FrameAnalysis, VideoViolationDetector};
use crate::detectors::{Detectors, Inference, Output, Prepare};
use crate::globals::FACE_DATABASE;
use crate::utils::BoundingBox;
use anyhow::{Result, anyhow};
use gst::prelude::*;
use gst_video::video_frame::Readable;
use gst_video::{VideoFrame, VideoFrameExt, gst};
use motion::motion::Motion;
use parking_lot::Mutex;
use project_config::global::PROJECT_CONFIG;
use rayon::join;
use rayon::prelude::*;
use std::sync::Arc;
use tokio::time::Instant;

enum DetectionMode {
    ObjectsDetection,
    MotionDetection,
}

fn detect_motion(frame: &VideoFrame<Readable>, motion: &mut Motion) -> Result<bool> {
    // println!("Detecting motion ...");
    let mat = Detectors::video_frame_to_mat(frame)?;
    let result = motion.predict(mat)?;
    Ok(result)
}

pub struct DetectionState {
    motion: Arc<Mutex<Motion>>,
    detectors: Detectors,
    mode: DetectionMode,
    viol_detector: VideoViolationDetector,
    frames_without_detection: usize,
    frames_without_motion: usize,
    detection_max_empty_frames: usize,
    motion_max_empty_frames: usize,
}

impl DetectionState {
    pub fn new(
        motion: Arc<Mutex<Motion>>,
        detectors: Detectors,
        viol_detector: VideoViolationDetector,
        detection_max_empty_frames: usize,
        motion_max_empty_frames: usize,
    ) -> Self {
        Self {
            motion,
            detectors,
            mode: DetectionMode::ObjectsDetection,
            viol_detector,
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
        &mut self,
        frame: &VideoFrame<Readable>,
        bounding_box: &Arc<Mutex<Vec<(BoundingBox, usize, f32)>>>,
    ) -> Result<bool> {
        // let start = Instant::now();

        let image = Detectors::convert_gst_image_to_dynamic(frame)?;

        let prepared = Detectors::prepare_dynamic_image_for_yolo11s(&image)
            .map_err(|e| anyhow!("prepare_dynamic_image failed: {:?}", e))?;

        let (persons_res, faces_res): (
            Result<Vec<(BoundingBox, usize, f32)>>,
            Result<Vec<(BoundingBox, usize, f32)>>,
        ) = join(
            || -> Result<Vec<(BoundingBox, usize, f32)>> {
                if let Some(person_model) = &self.detectors.person {
                    let session = person_model.get_session();

                    let inf = Detectors::yolo11_inference(session, prepared.clone())?;
                    let persons = Detectors::process_yolo11s_output(
                        inf,
                        640f32,
                        640f32,
                        None,
                        Some(&[0_usize]),
                    )?;

                    Ok(persons)
                } else {
                    Ok(Vec::new())
                }
            },
            || -> Result<Vec<(BoundingBox, usize, f32)>> {
                if let Some(face_models) = &self.detectors.face {
                    let det_session = face_models.detection.get_session();

                    let inf = Detectors::yolo11_inference(det_session, prepared.clone())?;

                    let faces =
                        Detectors::process_yolo11s_output(inf, 640f32, 640f32, Some(0.4), None)?;

                    Ok(faces)
                } else {
                    Ok(Vec::new())
                }
            },
        );

        let persons = persons_res?;
        let mut faces = faces_res?;
        let mut recognized_faces = Vec::new();

        let face_embeddings: Vec<_> = if let Some(face_models) = &self.detectors.face {
            let model_session = face_models.recognition.get_session();
            faces
                .par_iter()
                .filter_map(|(bbox, _class, _prob)| Detectors::crop_face(&image, bbox).ok())
                .map(|crop| {
                    // TODO: get WxH from config
                    let width = &PROJECT_CONFIG.face_recognition_model.input_width;
                    let height = &PROJECT_CONFIG.face_recognition_model.input_height;

                    let img = Detectors::resize(&crop, *width, *height)?;
                    let prep = Detectors::preprocess_image_for_recognition(&img)
                        .map_err(|e| anyhow!("prep failed: {:?}", e))?;
                    Detectors::extract_face_embedding(model_session, prep)
                })
                .collect()
        } else {
            Vec::new()
        };

        if !face_embeddings.is_empty() {
            let face_database = &FACE_DATABASE;

            for (face_idx, face_embedding_result) in face_embeddings.iter().enumerate() {
                match face_embedding_result {
                    Ok(face_embedding) => {
                        let mut best_match: Option<(String, f32)> = None;
                        let similarity_threshold =
                            &PROJECT_CONFIG.face_database.similarity_threshold;

                        for person in &face_database.people {
                            let mut max_similarity = 0.0f32;

                            // for person_embedding in &person.emb {
                            //     match Detectors::cosine_similarity(face_embedding, person_embedding)
                            //     {
                            //         Ok(similarity) => {
                            //             if similarity > max_similarity {
                            //                 max_similarity = similarity;
                            //             }
                            //         }
                            //         Err(e) => {
                            //             println!("Error to cosine_similarity : {:?}", e);
                            //         }
                            //     }
                            // }

                            match Detectors::cosine_similarity(
                                face_embedding,
                                &person.get_best_embedding(),
                            ) {
                                Ok(similarity) => {
                                    if similarity > max_similarity {
                                        max_similarity = similarity;
                                    }
                                }
                                Err(e) => {
                                    println!("Error to cosine_similarity : {:?}", e);
                                }
                            }

                            if max_similarity > *similarity_threshold {
                                match &best_match {
                                    Some((_, current_best_similarity)) => {
                                        if max_similarity > *current_best_similarity {
                                            best_match =
                                                Some((person.name.clone(), max_similarity));
                                        }
                                    }
                                    None => {
                                        best_match = Some((person.name.clone(), max_similarity));
                                    }
                                }
                            }
                        }

                        match best_match {
                            Some((name, similarity)) => {
                                recognized_faces.push(name.trim().to_string());
                                faces[face_idx].0.label =
                                    Some(format!("{} ({:.0}%)", name, similarity * 100.0));
                                println!(
                                    "Face {} recognized how: {} (cosine_similarity: {:.2}%)",
                                    face_idx,
                                    name,
                                    similarity * 100.0
                                );
                            }
                            None => {
                                // println!("Face {} not recognized", face_idx);
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error to get face embedding {}: {:?}", face_idx, e);
                    }
                }
            }
        }

        let mut boxes = bounding_box.lock();

        boxes.clear();

        boxes.extend(persons.clone());
        boxes.extend(faces.clone());

        let frame_analysis = FrameAnalysis {
            people_count: persons.len(),
            faces_detected: faces.len(),
            recognized_faces,
        };

        self.viol_detector.add_frame_analysis(frame_analysis);


        // self.viol_detector.finalize_and_print_report();
        // let duration = start.elapsed();

        // println!("Time elapsed in detect_objects is: {:?}", duration);

        Ok(!persons.is_empty())
    }

    pub fn finalize_detection(&mut self) {
        self.viol_detector.finalize_and_print_report();
    }
}
