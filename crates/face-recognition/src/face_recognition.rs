use anyhow::Result;
use detection::utils::BoundingBox;
use image::DynamicImage;
use ndarray::Array4;
use ort::inputs;
use ort::session::Session;
use rayon::prelude::*;

pub fn preprocess_image_for_recognition(
    image: &DynamicImage,
) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
    let img_rgb = match image {
        DynamicImage::ImageRgb8(rgb_img) => rgb_img,
        _ => return Err("Expected RGB8 format".into()),
    };

    let raw_data = img_rgb.as_raw();
    let channel_size = 160 * 160;
    let total_size = 3 * channel_size;
    let mut flat_data = vec![0.0f32; total_size];

    let (r_channel, rest) = flat_data.split_at_mut(channel_size);
    let (g_channel, b_channel) = rest.split_at_mut(channel_size);

    let r_rows = r_channel.par_chunks_mut(160);
    let g_rows = g_channel.par_chunks_mut(160);
    let b_rows = b_channel.par_chunks_mut(160);

    r_rows
        .zip(g_rows)
        .zip(b_rows)
        .enumerate()
        .for_each(|(y, ((r_row, g_row), b_row))| {
            let row_offset = y * 160 * 3;

            for x in 0..160 {
                let pixel_pos = row_offset + x * 3;

                if pixel_pos + 2 < raw_data.len() {
                    r_row[x] = (raw_data[pixel_pos] as f32 - 127.5) / 128.0;
                    g_row[x] = (raw_data[pixel_pos + 1] as f32 - 127.5) / 128.0;
                    b_row[x] = (raw_data[pixel_pos + 2] as f32 - 127.5) / 128.0;
                }
            }
        });

    Ok(Array4::from_shape_vec((1, 3, 160, 160), flat_data)?)
}

pub fn crop_face(image: &DynamicImage, bbox: &BoundingBox) -> Result<DynamicImage> {
    let padding = 0.0;
    let x1 = (bbox.x1 - padding).max(0.0) as u32;
    let y1 = (bbox.y1 - padding).max(0.0) as u32;
    let x2 = (bbox.x2 + padding).min(image.width() as f32) as u32;
    let y2 = (bbox.y2 + padding).min(image.height() as f32) as u32;

    Ok(image.crop_imm(x1, y1, x2 - x1, y2 - y1))
}

pub fn crop_multiple_boxes(
    frame: &DynamicImage,
    boxes: &[BoundingBox],
) -> Result<Vec<DynamicImage>> {
    let mut results = Vec::with_capacity(boxes.len());

    for bbox in boxes {
        results.push(crop_face(frame, bbox)?);
    }

    Ok(results)
}

pub fn resize(frame: &DynamicImage, width: u32, height: u32) -> Result<DynamicImage> {
    // TODO: use a faster way
    Ok(frame.resize_exact(width, height, image::imageops::FilterType::Nearest))
}

pub fn extract_face_embedding(session: &Session, input_tensor: Array4<f32>) -> Result<Vec<f32>> {
    // only for current model (face-recognition.onnx). In the future, it is necessary to use universal method

    let input = inputs!["input.1"=>input_tensor]?;

    let outputs = session.run(input)?;

    let embedding_array = outputs["1197"]
        .try_extract_tensor::<f32>()?
        .t()
        .into_owned();

    let embedding: Vec<f32> = embedding_array.into_iter().collect();

    let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    let normalized_embedding: Vec<f32> = if norm > 0.0 {
        embedding.iter().map(|x| x / norm).collect()
    } else {
        embedding
    };

    Ok(normalized_embedding)
}

pub fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> f32 {
    if vec1.len() != vec2.len() {
        return 0.0;
    }

    let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
    let norm1: f32 = vec1.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm2: f32 = vec2.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm1 == 0.0 || norm2 == 0.0 {
        return 0.0;
    }

    dot_product / (norm1 * norm2)
}
