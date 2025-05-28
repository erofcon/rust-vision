use anyhow::{Result, anyhow};
use detection::inference::run;
use detection::model::Model;
use gst_video::video_frame::Readable;
use gst_video::{VideoFrame, VideoFrameExt};
use motion::motion::Motion;
use opencv::core::{AlgorithmHint, CV_8UC3, CV_8UC4, Mat};
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard};

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
        gst_video::VideoFormat::Bgra => unsafe {
            let mut mat = Mat::new_rows_cols_with_data_unsafe_def(
                height,
                width,
                CV_8UC4,
                data.as_ptr() as *mut _,
            )?;
            mat.clone()
        },
        gst_video::VideoFormat::Rgb => unsafe {
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
        gst_video::VideoFormat::Bgr => unsafe {
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

fn detect_objects(frame: &VideoFrame<Readable>, model: RwLockReadGuard<Model>) -> Result<bool> {
    let result = run(model.get_session(), frame, 192, 180, 640, 640, Some(&[0]))?;
    println!("Detected objects: {:?}", result.len());
    println!("Detecting objects ...");
    Ok(false)
}

fn detect_motion(frame: &VideoFrame<Readable>, mut motion: MutexGuard<Motion>) -> Result<bool> {
    println!("Detecting motion ...");
    let mat = video_frame_to_mat(frame)?;

    let result = motion.predict(mat)?;

    Ok(result)
}

pub struct DetectionState {
    motion: Arc<Mutex<Motion>>,
    model: Arc<RwLock<Model>>,
    mode: DetectionMode,
    frames_without_detection: usize,
    frames_without_motion: usize,
    detection_max_empty_frames: usize,
    motion_max_empty_frames: usize,
}

impl DetectionState {
    pub fn new(
        motion: Arc<Mutex<Motion>>,
        model: Arc<RwLock<Model>>,
        detection_max_empty_frames: usize,
        motion_max_empty_frames: usize,
    ) -> Self {
        Self {
            motion,
            model,
            mode: DetectionMode::ObjectsDetection, // Start with person detection
            frames_without_detection: 0,
            frames_without_motion: 0,
            detection_max_empty_frames,
            motion_max_empty_frames,
        }
    }

    pub fn process_frame(&mut self, frame: &VideoFrame<Readable>) -> Result<bool> {
        match self.mode {
            DetectionMode::ObjectsDetection => {
                if let Ok(model) = self.model.read() {
                    let detected = detect_objects(frame, model)?;

                    return if detected {
                        self.frames_without_detection = 0;
                        Ok(true)
                    } else {
                        self.frames_without_detection += 1;
                        if self.frames_without_detection >= self.detection_max_empty_frames {
                            // TODO: add to log
                            self.mode = DetectionMode::MotionDetection;
                            self.frames_without_detection = 0;
                        }
                        Ok(false)
                    };
                }

                Ok(false)
            }
            DetectionMode::MotionDetection => {
                if let Ok(motion) = self.motion.lock() {
                    let motion_detected = detect_motion(frame, motion)?;

                    if motion_detected {
                        // TODO: add to log
                        self.frames_without_motion += 1;
                        if self.frames_without_motion >= self.motion_max_empty_frames {
                            self.mode = DetectionMode::ObjectsDetection;
                            self.frames_without_motion = 0;
                            if let Ok(model) = self.model.read() {
                                return detect_objects(frame, model);
                            }
                        }
                    } else {
                        self.frames_without_motion = 0;
                    }

                    Ok(false)
                } else {
                    Ok(false)
                }
            }
        }
    }
}