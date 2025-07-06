use crate::detection::{Detectors, Prepare};
use anyhow::Result;
use image::{DynamicImage, RgbImage};

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

pub fn img_to_rgb8(image: &DynamicImage, width: u32, height: u32) -> Result<RgbImage> {
    let image: DynamicImage = if image.width() != width || image.height() != height {
        Detectors::resize(image, width, height)?
    } else {
        image.clone()
    };

    let img_rgb: RgbImage = match image {
        DynamicImage::ImageRgb8(buf) => buf.clone(),
        _ => image.to_rgb8(),
    };

    Ok(img_rgb)
}
