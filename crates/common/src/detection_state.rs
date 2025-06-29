use anyhow::{Error, Result, anyhow};
use detection::inference::{prepare_image, run};
use detection::model::Model;
use gst_video::video_frame::Readable;
use gst_video::{VideoFrame, VideoFrameExt};
use motion::motion::Motion;
use ndarray::Array4;
use opencv::core::{AlgorithmHint, CV_8UC3, CV_8UC4, Mat};
use parking_lot::{Mutex, RwLock};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::sync::{Arc, mpsc};
use std::thread;
use tokio::runtime::Runtime;
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

fn detect_objects(
    frame: &VideoFrame<Readable>,
    object_detection_model: &Model,
    face_detection_model: &Model,
) -> Result<bool> {
    //50-52 ms

    let start_time = Instant::now();

    let tensor = prepare_image(frame)?;
    let tensor_arc = Arc::new(tensor);

    let tasks = vec![
        (object_detection_model.clone(), Arc::clone(&tensor_arc)),
        (face_detection_model.clone(), Arc::clone(&tensor_arc)),
    ];

    let results: Result<Vec<_>, _> = tasks
        .into_par_iter()
        .map(|(model, tensor)| {
            run(
                model.get_session(),
                (*tensor).clone(),
                1920,
                1080,
                640,
                640,
                Some(&[0]),
            )
        })
        .collect();

    let results = results?;

    let duration = start_time.elapsed();
    println!("Total time: {:?}", duration);

    Ok(true)
}

fn detect_objects_v2(
    frame: &VideoFrame<Readable>,
    object_detection_model: &Model,
    face_detection_model: &Model,
) -> Result<bool> {
    //52-55 ms

    let start_time = Instant::now();
    let tensor = prepare_image(frame)?;

    let (object_result, face_result) = std::thread::scope(|s| {
        let object_handle = s.spawn(|| {
            run(
                object_detection_model.get_session(),
                tensor.clone(),
                1920,
                1080,
                640,
                640,
                Some(&[0]),
            )
        });

        let face_handle = s.spawn(|| {
            run(
                face_detection_model.get_session(),
                tensor.clone(),
                1920,
                1080,
                640,
                640,
                Some(&[0]),
            )
        });

        (object_handle.join().unwrap(), face_handle.join().unwrap())
    });

    let obj_detection = object_result?;
    let face_detection = face_result?;

    let duration = start_time.elapsed();
    println!("Total time: {:?}", duration);

    Ok(true)
}

fn detect_motion(frame: &VideoFrame<Readable>, motion: &mut Motion) -> Result<bool> {
    println!("Detecting motion ...");
    let mat = video_frame_to_mat(frame)?;
    let result = motion.predict(mat)?;
    Ok(result)
}

pub struct DetectionState {
    motion: Arc<Mutex<Motion>>,
    object_detection_model: Arc<RwLock<Model>>,
    face_detection_model: Arc<RwLock<Model>>,
    mode: DetectionMode,
    frames_without_detection: usize,
    frames_without_motion: usize,
    detection_max_empty_frames: usize,
    motion_max_empty_frames: usize,
}

impl DetectionState {
    pub fn new(
        motion: Arc<Mutex<Motion>>,
        object_detection_model: Arc<RwLock<Model>>,
        face_detection_model: Arc<RwLock<Model>>,
        detection_max_empty_frames: usize,
        motion_max_empty_frames: usize,
    ) -> Self {
        Self {
            motion,
            object_detection_model,
            face_detection_model,
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
                let object_detection_model = self.object_detection_model.read();
                let face_detection_model = self.face_detection_model.read();
                let detected =
                    detect_objects(frame, &*object_detection_model, &*face_detection_model)?;

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
                    let object_detection_model = self.object_detection_model.read();
                    let face_detection_model = self.face_detection_model.read();
                    return detect_objects(frame, &*object_detection_model, &*face_detection_model);
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
}
