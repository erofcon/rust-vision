use crate::detectors::{Detectors, Prepare};
use crate::utils::{BoundingBox, Detection, img_to_rgb8};
use anyhow::{Result, anyhow};
use fast_image_resize::images::Image;
use fast_image_resize::{PixelType, Resizer};
use gst_video::video_frame::Readable;
use gst_video::{VideoFormat, VideoFrame, VideoFrameExt};
use image::{DynamicImage, ImageBuffer, Rgb};
use ndarray::{Array, Array4, ArrayBase, Axis, Dim, Ix, OwnedRepr};
use opencv::core::{
    AlgorithmHint, CV_8UC3, CV_8UC4, Mat, MatTraitConst, MatTraitConstManual, Point2f, Rect,
    Scalar, Size,
};
use opencv::imgproc;
use project_config::global::PROJECT_CONFIG;
use rayon::prelude::*;


impl Prepare for Detectors {
    fn prepare_dynamic_image_for_yolo11s(
        image: &DynamicImage,
    ) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
        let img_rgb = img_to_rgb8(image, 640, 640)?;

        let (width, height) = img_rgb.dimensions();

        let width = width as usize;
        let height = height as usize;

        let raw_data = img_rgb.as_raw();

        let channel_size = width * height;
        let total_size = 3 * channel_size;
        let mut flat_data = vec![0.0f32; total_size];

        let (r_channel, rest) = flat_data.split_at_mut(channel_size);
        let (g_channel, b_channel) = rest.split_at_mut(channel_size);

        let r_rows = r_channel.par_chunks_mut(width);
        let g_rows = g_channel.par_chunks_mut(width);
        let b_rows = b_channel.par_chunks_mut(width);

        r_rows
            .zip(g_rows)
            .zip(b_rows)
            .enumerate()
            .for_each(|(y, ((r_row, g_row), b_row))| {
                let row_offset = y * width * 3;

                for x in 0..width {
                    let pixel_pos = row_offset + x * 3;

                    if pixel_pos + 2 < raw_data.len() {
                        r_row[x] = raw_data[pixel_pos] as f32 / 255.0;
                        g_row[x] = raw_data[pixel_pos + 1] as f32 / 255.0;
                        b_row[x] = raw_data[pixel_pos + 2] as f32 / 255.0;
                    }
                }
            });

        let input = Array4::from_shape_vec((1, 3, height, width), flat_data)?;

        Ok(input)
    }

    fn preprocess_image_for_recognition(
        image: &DynamicImage,
    ) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
        let img_rgb = img_to_rgb8(image, 160, 160)?;

        let (width, height) = img_rgb.dimensions();
        let width = width as usize;
        let height = height as usize;

        let raw_data = img_rgb.as_raw();

        let channel_size = width * height;
        let total_size = 3 * channel_size;
        let mut flat_data = vec![0.0f32; total_size];

        let (r_channel, rest) = flat_data.split_at_mut(channel_size);
        let (g_channel, b_channel) = rest.split_at_mut(channel_size);

        let r_rows = r_channel.par_chunks_mut(width);
        let g_rows = g_channel.par_chunks_mut(width);
        let b_rows = b_channel.par_chunks_mut(width);

        r_rows
            .zip(g_rows)
            .zip(b_rows)
            .enumerate()
            .for_each(|(y, ((r_row, g_row), b_row))| {
                let row_offset = y * width * 3;

                for x in 0..width {
                    let pixel_pos = row_offset + x * 3;

                    if pixel_pos + 2 < raw_data.len() {
                        r_row[x] = (raw_data[pixel_pos] as f32 - 127.5) / 128.0;
                        g_row[x] = (raw_data[pixel_pos + 1] as f32 - 127.5) / 128.0;
                        b_row[x] = (raw_data[pixel_pos + 2] as f32 - 127.5) / 128.0;
                    }
                }
            });

        Ok(Array4::from_shape_vec((1, 3, height, width), flat_data)?)
    }

    fn preprocess_image_for_recognition_mat(
        image: &Mat,
    ) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
        let mut rgb_image = Mat::default();
        if image.channels() == 3 {
            imgproc::cvt_color(
                image,
                &mut rgb_image,
                imgproc::COLOR_BGR2RGB,
                0,
                AlgorithmHint::ALGO_HINT_DEFAULT,
            )?;
        } else {
            rgb_image = image.clone();
        }

        // Get image dimensions
        let width = rgb_image.cols() as usize;
        let height = rgb_image.rows() as usize;

        // Convert Mat to Vec<u8>
        let raw_data: Vec<u8> = rgb_image.data_bytes()?.to_vec();

        let channel_size = width * height;
        let total_size = 3 * channel_size;
        let mut flat_data = vec![0.0f32; total_size];

        let (r_channel, rest) = flat_data.split_at_mut(channel_size);
        let (g_channel, b_channel) = rest.split_at_mut(channel_size);

        let r_rows = r_channel.par_chunks_mut(width);
        let g_rows = g_channel.par_chunks_mut(width);
        let b_rows = b_channel.par_chunks_mut(width);

        r_rows
            .zip(g_rows)
            .zip(b_rows)
            .enumerate()
            .for_each(|(y, ((r_row, g_row), b_row))| {
                let row_offset = y * width * 3;

                for x in 0..width {
                    let pixel_pos = row_offset + x * 3;

                    if pixel_pos + 2 < raw_data.len() {
                        r_row[x] = (raw_data[pixel_pos] as f32 - 127.5) / 128.0;
                        g_row[x] = (raw_data[pixel_pos + 1] as f32 - 127.5) / 128.0;
                        b_row[x] = (raw_data[pixel_pos + 2] as f32 - 127.5) / 128.0;
                    }
                }
            });

        Ok(Array4::from_shape_vec((1, 3, height, width), flat_data)?)
    }

    fn crop_face(image: &DynamicImage, bbox: &BoundingBox) -> Result<DynamicImage> {
        let padding = 0.0;
        let x1 = (bbox.x1 - padding).max(0.0) as u32;
        let y1 = (bbox.y1 - padding).max(0.0) as u32;
        let x2 = (bbox.x2 + padding).min(image.width() as f32) as u32;
        let y2 = (bbox.y2 + padding).min(image.height() as f32) as u32;

        Ok(image.crop_imm(x1, y1, x2 - x1, y2 - y1))
    }

    fn crop_detection(input_img: &Mat, detection: &Detection) -> Result<Mat> {
        let [x_min, y_min, x_max, y_max] = detection.bbox;

        let x = x_min.max(0.0) as i32;
        let y = y_min.max(0.0) as i32;
        let width = (x_max - x_min).min(input_img.cols() as f32 - x as f32) as i32;
        let height = (y_max - y_min).min(input_img.rows() as f32 - y as f32) as i32;

        let rect = Rect::new(x, y, width, height);
        let roi = Mat::roi(input_img, rect)?;

        let mut cropped = Mat::default();
        roi.copy_to(&mut cropped)?;

        Ok(cropped)
    }

    fn align_face(face: &Mat, detection: &Detection) -> Result<Mat> {
        let mut left_eye: Option<Point2f> = None;
        let mut right_eye: Option<Point2f> = None;

        let bbox = &detection.bbox;
        let face_x_min = bbox[0];
        let face_y_min = bbox[1];

        if detection.keypoints.len() >= 2 {
            if detection.keypoints[0][2] > 0.5 {
                left_eye = Some(Point2f::new(
                    detection.keypoints[0][0] - face_x_min,
                    detection.keypoints[0][1] - face_y_min,
                ));
            }
            if detection.keypoints[1][2] > 0.5 {
                right_eye = Some(Point2f::new(
                    detection.keypoints[1][0] - face_x_min,
                    detection.keypoints[1][1] - face_y_min,
                ));
            }
        }

        let mut aligned = Mat::default();

        if let (Some(left), Some(right)) = (left_eye, right_eye) {
            let dx = right.x - left.x;
            let dy = right.y - left.y;
            let angle = dy.atan2(dx) * 180.0 / std::f32::consts::PI;

            let center = Point2f::new((left.x + right.x) / 2.0, (left.y + right.y) / 2.0);
            let rotation_matrix =
                opencv::imgproc::get_rotation_matrix_2d(center, angle as f64, 1.0)?;

            imgproc::warp_affine(
                face,
                &mut aligned,
                &rotation_matrix,
                face.size()?,
                opencv::imgproc::INTER_LINEAR,
                opencv::core::BORDER_REFLECT_101,
                opencv::core::Scalar::default(),
            )?;
        } else {
            face.copy_to(&mut aligned)?;
        }

        Ok(aligned)
    }

    fn resize(frame: &DynamicImage, width: u32, height: u32) -> Result<DynamicImage> {
        let src_img = Image::from_vec_u8(
            frame.width(),
            frame.height(),
            frame.to_rgba8().into_raw(),
            PixelType::U8x4,
        )?;

        let mut dst_image = Image::new(width, height, PixelType::U8x4);

        let mut resize = Resizer::new();
        resize.resize(&src_img, &mut dst_image, None)?;

        Ok(DynamicImage::ImageRgba8(
            ImageBuffer::from_raw(width, height, dst_image.into_vec()).unwrap(),
        ))
    }

    fn convert_gst_image_to_dynamic(frame: &VideoFrame<Readable>) -> Result<DynamicImage> {
        let width = frame.width();
        let height = frame.height();
        let plane_data = frame.plane_data(0)?;
        let stride = frame.plane_stride()[0] as u32;

        let mut img_data = vec![0u8; (width * height * 3) as usize];

        img_data
            .par_chunks_mut((width * 3) as usize)
            .enumerate()
            .for_each(|(y, row_chunk)| {
                let row_start = (y as u32 * stride) as usize;

                for x in 0..width {
                    let pixel_start = row_start + (x * 3) as usize;
                    let img_pixel_start = (x * 3) as usize;

                    if pixel_start + 2 < plane_data.len() {
                        row_chunk[img_pixel_start] = plane_data[pixel_start + 2]; // R
                        row_chunk[img_pixel_start + 1] = plane_data[pixel_start + 1]; // G
                        row_chunk[img_pixel_start + 2] = plane_data[pixel_start]; // B
                    }
                }
            });

        let img_buffer = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(width, height, img_data)
            .ok_or("Failed to create RGB image buffer")
            .unwrap();

        Ok(DynamicImage::ImageRgb8(img_buffer))
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

    fn prepare_mat_img(
        img: &Mat,
        target_width: usize,
        target_height: usize,
    ) -> Result<ArrayBase<OwnedRepr<f32>, Dim<[Ix; 4]>>> {
        let mut model_input = Array::zeros((1, 3, target_width, target_height));

        // Получаем прямой доступ к данным изображения
        let img_data = img.data_bytes()?;
        let channels = img.channels() as usize;
        let step = img.step1(0)?;

        // Параллельная обработка строк
        model_input
            .axis_iter_mut(Axis(2))
            .into_par_iter()
            .enumerate()
            .for_each(|(y, mut row)| {
                let img_row = &img_data[y * step..];

                for x in 0..target_width {
                    let pixel_offset = x * channels;
                    if pixel_offset + 2 < img_row.len() {
                        row[[0, 0, x]] = img_row[pixel_offset + 2] as f32 / 255.0; // R
                        row[[0, 1, x]] = img_row[pixel_offset + 1] as f32 / 255.0; // G
                        row[[0, 2, x]] = img_row[pixel_offset] as f32 / 255.0; // B
                    }
                }
            });

        Ok(model_input)
    }

    fn resize_mat_with_padding(src: &Mat) -> Result<(Mat, f32, f32, f32)> {
        let src_h = src.rows() as f32;
        let src_w = src.cols() as f32;
        let target_width = PROJECT_CONFIG.face_detection_model.input_width.clone();
        let target_height = PROJECT_CONFIG.face_detection_model.input_height.clone();

        let scale = (target_width as f32 / src_w).min(target_height as f32 / src_h);

        let new_w = (src_w * scale) as i32;
        let new_h = (src_h * scale) as i32;

        let mut resized = Mat::default();
        opencv::imgproc::resize(
            src,
            &mut resized,
            Size::new(new_w, new_h),
            0.0,
            0.0,
            opencv::imgproc::INTER_LINEAR,
        )?;

        let mut padded = Mat::default();

        opencv::core::copy_make_border(
            &resized,
            &mut padded,
            (target_width - new_h) / 2,
            (target_width - new_h) / 2,
            (target_width - new_w) / 2,
            (target_width - new_w) / 2,
            opencv::core::BORDER_CONSTANT,
            Scalar::new(114.0, 114.0, 114.0, 0.0),
        )?;

        let pad_x = ((target_width - new_w) / 2) as f32;
        let pad_y = ((target_height - new_h) / 2) as f32;

        Ok((padded, scale, pad_x, pad_y))
    }

    fn resize_mat(img: &Mat, target_width: i32, target_height: i32) -> Result<Mat> {
        let mut resized = Mat::default();
        imgproc::resize(
            img,
            &mut resized,
            Size::new(target_width, target_height),
            0.0,
            0.0,
            imgproc::INTER_LINEAR,
        )?;

        Ok(resized)
    }
}
