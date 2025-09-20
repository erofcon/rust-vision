use anyhow::Result;
use gst_video::video_frame::Readable;
use gst_video::{VideoFrame, VideoFrameExt};
use image::{DynamicImage, ImageBuffer, Rgb};
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

pub fn intersection(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    (box1.x2.min(box2.x2) - box1.x1.max(box2.x1)) * (box1.y2.min(box2.y2) - box1.y1.max(box2.y1))
}

pub fn union(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    ((box1.x2 - box1.x1) * (box1.y2 - box1.y1)) + ((box2.x2 - box2.x1) * (box2.y2 - box2.y1))
        - intersection(box1, box2)
}
pub fn convert_gst_image_to_dynamic(frame: &VideoFrame<Readable>) -> Result<DynamicImage> {
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
