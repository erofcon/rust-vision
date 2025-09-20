use crate::utils::{BoundingBox, intersection, union};
use anyhow::Result;
use gst_video::video_frame::Readable;
use gst_video::{VideoFrame, VideoFrameExt};
use image::DynamicImage;
use ndarray::{Array, Array4, Axis, IxDyn, s};
use opencv::prelude::{MatTraitConst, MatTraitConstManual};
use ort::inputs;
use ort::session::Session;
use rayon::prelude::*;

pub fn yolov11_inference(
    session: &Session,
    frame: Array<f32, ndarray::Dim<[usize; 4]>>,
) -> Result<Array<f32, IxDyn>> {
    let input = inputs!["images"=>frame]?;

    let outputs = session.run(input)?;

    Ok(outputs["output0"]
        .try_extract_tensor::<f32>()?
        .t()
        .into_owned())
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

pub fn process_output(
    output: Array<f32, IxDyn>,
    out_classes: Option<&[usize]>,
) -> Result<Vec<(BoundingBox, usize, f32)>> {
    let prob_threshold = 0.45;
    let iou_threshold = 0.7;

    let sliced = output.slice(s![.., .., 0]);

    let mut boxes: Vec<_> = sliced
        .axis_iter(Axis(0))
        .into_par_iter()
        .filter_map(|row| {
            let (class_id, &prob) = row
                .iter()
                .skip(4)
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())?;

            if prob < prob_threshold {
                return None;
            }

            if let Some(set) = out_classes {
                if !set.contains(&class_id) {
                    return None;
                }
            }

            let xc = row[0_usize];
            let yc = row[1_usize];
            let w = row[2_usize];
            let h = row[3_usize];

            let bbox = BoundingBox {
                x1: xc - w / 2.,
                y1: yc - h / 2.,
                x2: xc + w / 2.,
                y2: yc + h / 2.,
            };
            Some((bbox, class_id, prob))
        })
        .collect();

    boxes.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    let mut selected = Vec::with_capacity(boxes.len());
    while let Some(current) = boxes.pop() {
        selected.push(current);
        boxes.retain(|b| {
            let iou = intersection(&current.0, &b.0) / union(&current.0, &b.0);
            iou <= iou_threshold
        });
    }

    Ok(selected)
}

pub fn prepare_gst_image(
    frame: &VideoFrame<Readable>,
) -> Result<Array<f32, ndarray::Dim<[usize; 4]>>> {
    // let start_time = Instant::now();

    let frame_width = frame.width() as usize;
    let frame_height = frame.height() as usize;
    let channel_size = frame_width * frame_height;
    let total_size = 3 * channel_size;
    let mut flat_data = vec![0.0f32; total_size];

    let stride = frame.plane_stride()[0] as usize;
    let data = frame.plane_data(0)?;

    let (r_channel, rest) = flat_data.split_at_mut(channel_size);
    let (g_channel, b_channel) = rest.split_at_mut(channel_size);

    let r_rows = r_channel.par_chunks_mut(frame_width);
    let g_rows = g_channel.par_chunks_mut(frame_width);
    let b_rows = b_channel.par_chunks_mut(frame_width);

    r_rows
        .zip(g_rows)
        .zip(b_rows)
        .enumerate()
        .for_each(|(y, ((r_row, g_row), b_row))| {
            let row_offset = y * stride;
            for x in 0..frame_width {
                let pixel_pos = row_offset + x * 3;

                if pixel_pos + 2 < data.len() {
                    r_row[x] = data[pixel_pos] as f32 / 255.0;
                    g_row[x] = data[pixel_pos + 1] as f32 / 255.0;
                    b_row[x] = data[pixel_pos + 2] as f32 / 255.0;
                }
            }
        });

    let input = Array::from_shape_vec((1, 3, frame_height, frame_width), flat_data)?;

    // let duration = start_time.elapsed();
    //
    // println!("{:?}", duration);

    Ok(input)
}

pub fn prepare_dynamic_image(
    image: &DynamicImage,
) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
    let img_rgb = match image {
        DynamicImage::ImageRgb8(rgb_img) => rgb_img,
        _ => return Err("Expected RGB8 format".into()),
    };

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

pub fn preprocess_image_for_recognition(
    image: &DynamicImage,
) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
    let img_rgb = match image {
        DynamicImage::ImageRgb8(rgb_img) => rgb_img,
        _ => return Err("Expected RGB8 format".into()),
    };

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
