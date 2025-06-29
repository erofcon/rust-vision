use crate::utils::{BoundingBox, intersection, union};
use anyhow::Result;
use gst_video::video_frame::Readable;
use gst_video::{VideoFrame, VideoFrameExt};
use ndarray::{Array, Axis, IxDyn, s};
use opencv::prelude::{MatTraitConst, MatTraitConstManual};
use ort::inputs;
use ort::session::Session;
use rayon::prelude::*;
use std::time::Instant;

pub fn run(
    session: &Session,
    frame: Array<f32, ndarray::Dim<[usize; 4]>>,
    original_img_width: i32,
    original_img_height: i32,
    model_input_width: i32,
    model_input_height: i32,
    out_classes: Option<&[usize]>,
) -> Result<Vec<(BoundingBox, usize, f32)>> { //
    // let image = prepare_image(frame)?;

    let input = inputs!["images"=>frame]?;

    // let start_time = Instant::now();

    let output = {
        let outputs = session.run(input)?;

        outputs["output0"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned()
    };

    // let duration = start_time.elapsed();

    // println!("{:?}", duration);

    let out = process_output(
        output,
        original_img_width,
        original_img_height,
        model_input_width,
        model_input_height,
        out_classes,
    );
    //
    out
}

pub fn process_output(
    output: Array<f32, IxDyn>,
    original_img_width: i32,
    original_img_height: i32,
    model_input_width: i32,
    model_input_height: i32,
    out_classes: Option<&[usize]>,
) -> Result<Vec<(BoundingBox, usize, f32)>> {
    let scale_x = original_img_width as f32 / model_input_width as f32;
    let scale_y = original_img_height as f32 / model_input_height as f32;
    let prob_threshold = 0.35;
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

            let xc = row[0_usize] * scale_x;
            let yc = row[1_usize] * scale_y;
            let w = row[2_usize] * scale_x;
            let h = row[3_usize] * scale_y;
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

pub fn prepare_image(frame: &VideoFrame<Readable>) -> Result<Array<f32, ndarray::Dim<[usize; 4]>>> {
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
