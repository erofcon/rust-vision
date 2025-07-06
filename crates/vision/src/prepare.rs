use crate::detection::{Detectors, Prepare};
use crate::utils::{BoundingBox, img_to_rgb8};
use anyhow::Result;
use image::DynamicImage;
use ndarray::Array4;
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

    fn crop_face(image: &DynamicImage, bbox: &BoundingBox) -> Result<DynamicImage> {
        let padding = 0.0;
        let x1 = (bbox.x1 - padding).max(0.0) as u32;
        let y1 = (bbox.y1 - padding).max(0.0) as u32;
        let x2 = (bbox.x2 + padding).min(image.width() as f32) as u32;
        let y2 = (bbox.y2 + padding).min(image.height() as f32) as u32;

        Ok(image.crop_imm(x1, y1, x2 - x1, y2 - y1))
    }

    fn resize(frame: &DynamicImage, width: u32, height: u32) -> Result<DynamicImage> {
        // TODO: use a faster way
        Ok(frame.resize_exact(width, height, image::imageops::FilterType::Nearest))
    }
}
