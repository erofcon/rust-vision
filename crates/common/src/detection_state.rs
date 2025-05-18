use anyhow::{Result, anyhow};
use gst_video::video_frame::Readable;
use gst_video::{VideoFrame, VideoFrameExt};
use opencv::core::{AlgorithmHint, CV_8UC3, CV_8UC4, Mat, Vector};
use opencv::imgcodecs;
use std::ffi::c_void;

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

fn detect_objects(_frame: &VideoFrame<Readable>) -> Result<bool> {
    Ok(false)
}

fn detect_motion(frame: &VideoFrame<Readable>) -> Result<bool> {
    let mat = video_frame_to_mat(frame)?;


    Ok(false)
}

pub struct DetectionState {
    mode: DetectionMode,
    frames_without_detection: usize,
    max_empty_frames: usize,
}

impl DetectionState {
    pub fn new(max_empty_frames: usize) -> Self {
        Self {
            mode: DetectionMode::ObjectsDetection, // Start with person detection
            frames_without_detection: 0,
            max_empty_frames,
        }
    }

    pub fn process_frame(&mut self, frame: &VideoFrame<Readable>) -> Result<bool> {
        match self.mode {
            DetectionMode::ObjectsDetection => {
                let detected = detect_objects(frame)?;

                if detected {
                    self.frames_without_detection = 0;
                    Ok(true)
                } else {
                    self.frames_without_detection += 1;

                    if self.frames_without_detection >= self.max_empty_frames {
                        // TODO: add to log
                        println!(
                            "Switching to motion detection after {} frames without persons",
                            self.max_empty_frames
                        );
                        self.mode = DetectionMode::MotionDetection;
                        self.frames_without_detection = 0;
                    }
                    Ok(false)
                }
            }
            DetectionMode::MotionDetection => {
                println!("Motion detection");
                let motion_detected = detect_motion(frame)?;

                if motion_detected {
                    // TODO: add to log
                    // If motion detected, switch back to person detection
                    println!("Motion detected! Switching back to person detection");
                    self.mode = DetectionMode::ObjectsDetection;
                    self.frames_without_detection = 0;

                    return detect_objects(frame);
                }

                Ok(false)
            }
        }
    }
}
