use crate::detectors::{Detectors, Prepare};
use crate::utils::{BoundingBox, img_to_rgb8};
use anyhow::{Result, anyhow};
use fast_image_resize::images::Image;
use fast_image_resize::{PixelType, Resizer};
use gst_video::video_frame::Readable;
use gst_video::{VideoFormat, VideoFrame, VideoFrameExt};
use image::{DynamicImage, ImageBuffer, Rgb};
use ndarray::Array4;
use opencv::core::{AlgorithmHint, CV_8UC3, CV_8UC4, Mat};
use rayon::prelude::*;
use std::time::Instant;

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

    fn crop_face(image: &DynamicImage, bbox: &BoundingBox) -> Result<DynamicImage> {
        let padding = 0.0;
        let x1 = (bbox.x1 - padding).max(0.0) as u32;
        let y1 = (bbox.y1 - padding).max(0.0) as u32;
        let x2 = (bbox.x2 + padding).min(image.width() as f32) as u32;
        let y2 = (bbox.y2 + padding).min(image.height() as f32) as u32;

        Ok(image.crop_imm(x1, y1, x2 - x1, y2 - y1))
    }

    // fn resize(frame: &DynamicImage, width: u32, height: u32) -> Result<DynamicImage> {
    //     // 8-9 ms
    //     let start = Instant::now();
    //     // TODO: use a faster way
    //     let r = Ok(frame.resize_exact(width, height, image::imageops::FilterType::Lanczos3));
    //
    //     let duration = start.elapsed();
    //     println!("Resizing to: {:?}", duration);
    //
    //     r
    // }

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
}
