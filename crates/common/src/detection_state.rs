use crate::ort_session::Model;
use anyhow::{Error, Result, anyhow};
use detection::inference::{prepare_dynamic_image, run};
use detection::utils::{BoundingBox, convert_gst_image_to_dynamic};
use face_recognition::face_recognition::*;
use gst::prelude::*;
use gst_video::gst::Buffer;
use gst_video::video_frame::Readable;
use gst_video::{VideoFormat, VideoFrame, VideoFrameExt, VideoInfo, gst};
use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};
use motion::motion::Motion;
use ndarray::Array4;
use opencv::core::{AlgorithmHint, CV_8UC3, CV_8UC4, Mat};
use parking_lot::{Mutex, RwLock};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::sync::{Arc, mpsc};
use std::{fs, thread};
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
    face_recognition_model: &Model,
) -> Result<bool> {
    let start_time = Instant::now();

    let image = convert_gst_image_to_dynamic(frame)?;
    let duration = start_time.elapsed();

    println!("{:?}", duration);

    let tensor = prepare_dynamic_image(&image).unwrap();

    // face detection

    let results = run(
        face_detection_model.get_session(),
        tensor.clone(),
        1920,
        1080,
        640,
        640,
        Some(&[0]),
    )?;

    if !results.is_empty() {
        // recognition

        let crop = crop_face(&image, &results[0].0)?;
        let img = resize(&crop, 160, 160)?;

        let prepare = preprocess_image_for_recognition(&img).unwrap();
        let emb = extract_face_embedding(face_recognition_model.get_session(), prepare)?;

        println!("Emb: {:?}", emb.len());
        // let img_rgb = img.to_rgb8();

        println!("Cropped");
        crop.save("crop.png")?;
        img.save("resized.png")?;
    }

    Ok(true)
}

// pub fn preprocess_image_for_recognition(
//     image: &DynamicImage,
// ) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
//     // Прямой доступ к RGB8 данным без копирования
//     let img_rgb = match image {
//         DynamicImage::ImageRgb8(rgb_img) => rgb_img,
//         _ => return Err("Expected RGB8 format".into()),
//     };
//
//     let raw_data = img_rgb.as_raw();
//     let channel_size = 160 * 160;
//     let total_size = 3 * channel_size;
//     let mut flat_data = vec![0.0f32; total_size];
//
//     // Разделяем на каналы
//     let (r_channel, rest) = flat_data.split_at_mut(channel_size);
//     let (g_channel, b_channel) = rest.split_at_mut(channel_size);
//
//     // Параллельная обработка по строкам
//     let r_rows = r_channel.par_chunks_mut(160);
//     let g_rows = g_channel.par_chunks_mut(160);
//     let b_rows = b_channel.par_chunks_mut(160);
//
//     r_rows
//         .zip(g_rows)
//         .zip(b_rows)
//         .enumerate()
//         .for_each(|(y, ((r_row, g_row), b_row))| {
//             let row_offset = y * 160 * 3; // RGB данные идут подряд
//
//             for x in 0..160 {
//                 let pixel_pos = row_offset + x * 3;
//
//                 if pixel_pos + 2 < raw_data.len() {
//                     // Нормализация: (pixel - 127.5) / 128.0
//                     r_row[x] = (raw_data[pixel_pos] as f32 - 127.5) / 128.0;
//                     g_row[x] = (raw_data[pixel_pos + 1] as f32 - 127.5) / 128.0;
//                     b_row[x] = (raw_data[pixel_pos + 2] as f32 - 127.5) / 128.0;
//                 }
//             }
//         });
//
//     // Создаем массив с правильными размерами (1, 3, 160, 160)
//     let input = Array4::from_shape_vec((1, 3, 160, 160), flat_data)?;
//
//     Ok(input)
// }
//
// pub fn prepare_image_for_dynamic_image(
//     image: &DynamicImage,
// ) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
//     let img_rgb = match image {
//         DynamicImage::ImageRgb8(rgb_img) => rgb_img,
//         _ => return Err("Expected RGB8 format".into()),
//     };
//
//     let (width, height) = img_rgb.dimensions();
//     let width = width as usize;
//     let height = height as usize;
//
//     // Получаем прямой доступ к сырым данным
//     let raw_data = img_rgb.as_raw();
//
//     let channel_size = width * height;
//     let total_size = 3 * channel_size;
//     let mut flat_data = vec![0.0f32; total_size];
//
//     // Разделяем на каналы
//     let (r_channel, rest) = flat_data.split_at_mut(channel_size);
//     let (g_channel, b_channel) = rest.split_at_mut(channel_size);
//
//     // Параллельная обработка по строкам
//     let r_rows = r_channel.par_chunks_mut(width);
//     let g_rows = g_channel.par_chunks_mut(width);
//     let b_rows = b_channel.par_chunks_mut(width);
//
//     r_rows
//         .zip(g_rows)
//         .zip(b_rows)
//         .enumerate()
//         .for_each(|(y, ((r_row, g_row), b_row))| {
//             let row_offset = y * width * 3; // RGB данные идут подряд
//
//             for x in 0..width {
//                 let pixel_pos = row_offset + x * 3;
//
//                 if pixel_pos + 2 < raw_data.len() {
//                     r_row[x] = raw_data[pixel_pos] as f32 / 255.0;
//                     g_row[x] = raw_data[pixel_pos + 1] as f32 / 255.0;
//                     b_row[x] = raw_data[pixel_pos + 2] as f32 / 255.0;
//                 }
//             }
//         });
//
//     // Создаем массив с правильными размерами (1, 3, height, width)
//     let input = Array4::from_shape_vec((1, 3, height, width), flat_data)?;
//
//     Ok(input)
// }
//
// fn detect_objects_v2(
//     frame: &VideoFrame<Readable>,
//     object_detection_model: &Model,
//     face_detection_model: &Model,
// ) -> Result<bool> {
//     //52-55 ms
//
//     let start_time = Instant::now();
//     let tensor = prepare_image(frame)?;
//
//     let (object_result, face_result) = std::thread::scope(|s| {
//         let object_handle = s.spawn(|| {
//             run(
//                 object_detection_model.get_session(),
//                 tensor.clone(),
//                 1920,
//                 1080,
//                 640,
//                 640,
//                 Some(&[0]),
//             )
//         });
//
//         let face_handle = s.spawn(|| {
//             run(
//                 face_detection_model.get_session(),
//                 tensor.clone(),
//                 1920,
//                 1080,
//                 640,
//                 640,
//                 Some(&[0]),
//             )
//         });
//
//         (object_handle.join().unwrap(), face_handle.join().unwrap())
//     });
//
//     let obj_detection = object_result?;
//     let face_detection = face_result?;
//
//     let duration = start_time.elapsed();
//     println!("Total time: {:?}", duration);
//
//     Ok(true)
// }

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
    face_recognition_model: Arc<RwLock<Model>>,
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
        face_recognition_model: Arc<RwLock<Model>>,
        detection_max_empty_frames: usize,
        motion_max_empty_frames: usize,
    ) -> Self {
        Self {
            motion,
            object_detection_model,
            face_detection_model,
            face_recognition_model,
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
                let face_recognition_model = self.face_recognition_model.read();

                let detected = detect_objects(
                    frame,
                    &*object_detection_model,
                    &*face_detection_model,
                    &*face_recognition_model,
                )?;

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
                    let face_recognition_model = self.face_recognition_model.read();
                    return detect_objects(
                        frame,
                        &*object_detection_model,
                        &*face_detection_model,
                        &*face_recognition_model,
                    );
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
