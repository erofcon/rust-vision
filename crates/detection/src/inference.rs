use crate::utils::{BoundingBox, intersection, union};
use anyhow::Result;
use gst_video::gst_base::base_parse_frame::Overhead::Frame;
use gst_video::prelude::GstBinExt;
use gst_video::video_frame::Readable;
use gst_video::{VideoFrame, VideoFrameExt};
use ndarray::{Array, Axis, IxDyn, s};
use opencv::core::Mat;
use opencv::prelude::{MatTraitConst, MatTraitConstManual};
use ort::inputs;
use ort::session::Session;
use rayon::prelude::*;
use std::time::Instant;

pub fn run(
    session: &Session,
    frame: &VideoFrame<Readable>,
    original_img_width: i32,
    original_img_height: i32,
    model_input_width: i32,
    model_input_height: i32,
    out_classes: Option<&[usize]>,
) -> Result<Vec<(BoundingBox, usize, f32)>> {
    let image = prepare_image(frame)?;

    let input = inputs!["images"=>image]?;

    let output = {
        let outputs = session.run(input)?;

        outputs["output0"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned()
    };

    process_output(
        output,
        original_img_width,
        original_img_height,
        model_input_width,
        model_input_height,
        out_classes,
    )
}

fn process_output(
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

fn prepare_image(frame: &VideoFrame<Readable>) -> Result<Array<f32, ndarray::Dim<[usize; 4]>>> {
    let start_time = Instant::now();

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

    let duration = start_time.elapsed();

    println!("{:?}", duration);

    Ok(input)
}
/*
1.8387ms
2.134ms
2.3327ms
2.4853ms

*/

// rayon
// pub fn detection_with_mat(session: &Session, frame: &Mat) -> Result<()> {
//     // use rayon::prelude::*;
//     let start_time = Instant::now();
//     // Get frame dimensions
//     let width = frame.cols() as usize;
//     let height = frame.rows() as usize;
//     let channels = frame.channels() as usize;
//
//     // Check supported formats
//     if channels != 3 && channels != 4 {
//         return Err(anyhow::anyhow!(
//             "Unsupported image format. Expected 3 or 4 channels."
//         ));
//     }
//
//     // Pre-allocate flat data vector
//     let channel_size = width * height;
//     let total_size = 3 * channel_size;
//     let mut flat_data = vec![0.0f32; total_size];
//
//     // Process data in parallel by chunks (rows)
//     flat_data
//         .par_chunks_mut(width * 3)
//         .enumerate()
//         .for_each(|(y, row_chunk)| {
//             let (r_row, rest) = row_chunk.split_at_mut(width);
//             let (g_row, b_row) = rest.split_at_mut(width);
//
//             if channels == 4 {
//                 // 4-channel processing (BGRA)
//                 for x in 0..width {
//                     let pixel = frame
//                         .at_2d::<opencv::core::Vec4b>(y as i32, x as i32)
//                         .unwrap();
//                     r_row[x] = pixel[2] as f32 / 255.0; // R
//                     g_row[x] = pixel[1] as f32 / 255.0; // G
//                     b_row[x] = pixel[0] as f32 / 255.0; // B
//                 }
//             } else {
//                 for x in 0..width {
//                     let pixel = frame
//                         .at_2d::<opencv::core::Vec3b>(y as i32, x as i32)
//                         .unwrap();
//                     r_row[x] = pixel[2] as f32 / 255.0; // R
//                     g_row[x] = pixel[1] as f32 / 255.0; // G
//                     b_row[x] = pixel[0] as f32 / 255.0; // B
//                 }
//             }
//         });
//
//     let model_input = Array::from_shape_vec((1, 3, height, width), flat_data)?;
//
//     let duration = start_time.elapsed();
//
//     println!("{:?}", duration);
//
//     Ok(())
// }

// pub fn detection_with_mat(session: &Session, frame: &Mat) -> Result<()> {
//     // prepare
//
//     let width = frame.cols();
//     let height = frame.rows();
//
//     let channels = frame.channels();
//
//     let mut model_input = Array::zeros((1, 3, width as usize, height as usize));
//
//     if channels == 4 {
//         for y in 0..height {
//             for x in 0..width {
//                 let pixel = frame.at_2d::<opencv::core::Vec4b>(y, x)?;
//                 // Extract BGR channels and ignore Alpha (pixel[3])
//                 model_input[[0, 0, y as usize, x as usize]] = pixel[2] as f32 / 255.; // R
//                 model_input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.; // G
//                 model_input[[0, 2, y as usize, x as usize]] = pixel[0] as f32 / 255.; // B
//             }
//         }
//     } else if channels == 3 {
//         for y in 0..height {
//             for x in 0..width {
//                 let pixel = frame.at_2d::<opencv::core::Vec3b>(y, x)?;
//                 model_input[[0, 0, y as usize, x as usize]] = pixel[2] as f32 / 255.; // R
//                 model_input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.; // G
//                 model_input[[0, 2, y as usize, x as usize]] = pixel[0] as f32 / 255.; // B
//             }
//         }
//     } else {
//         return Err(anyhow::anyhow!(
//             "Unsupported image format. Expected 3 or 4 channels."
//         ));
//     }
//
//     // let input = inputs!["images"=>model_input]?;
//     //
//     // let output = {
//     //     let outputs = session.run(input)?;
//     //
//     //     outputs["output0"]
//     //         .try_extract_tensor::<f32>()?
//     //         .t()
//     //         .into_owned()
//     // };
//     //
//
//     Ok(())
// }

// pub fn detection_with_mat(session: &Session, frame: &Mat) -> Result<()> {
//     let start_time = Instant::now();
//
//     // Get frame dimensions
//     let width = frame.cols() as usize;
//     let height = frame.rows() as usize;
//     let channels = frame.channels() as usize;
//
//     // Check supported formats
//     if channels != 3 && channels != 4 {
//         return Err(anyhow::anyhow!(
//             "Unsupported image format. Expected 3 or 4 channels."
//         ));
//     }
//
//     // Pre-allocate flat data vector
//     let channel_size = width * height;
//     let total_size = 3 * channel_size;
//     let mut flat_data = vec![0.0f32; total_size];
//
//     // Process rows
//     for y in 0..height {
//         // Process each row at once
//         if channels == 4 {
//             // Get a row of data to avoid repeated calls to at_2d
//             let row = frame.row(y as i32)?;
//             let row_data = row.data_typed::<opencv::core::Vec4b>()?;
//
//             // Process all pixels in the row
//             for x in 0..width {
//                 let pixel = row_data[x];
//
//                 // Calculate indices directly
//                 let r_idx = y * width + x;
//                 let g_idx = channel_size + y * width + x;
//                 let b_idx = 2 * channel_size + y * width + x;
//
//                 flat_data[r_idx] = pixel[2] as f32 / 255.0; // R
//                 flat_data[g_idx] = pixel[1] as f32 / 255.0; // G
//                 flat_data[b_idx] = pixel[0] as f32 / 255.0; // B
//             }
//         } else {
//             // Get a row of data to avoid repeated calls to at_2d
//             let row = frame.row(y as i32)?;
//             let row_data = row.data_typed::<opencv::core::Vec3b>()?;
//
//             // Process all pixels in the row
//             for x in 0..width {
//                 let pixel = row_data[x];
//
//                 // Calculate indices directly
//                 let r_idx = y * width + x;
//                 let g_idx = channel_size + y * width + x;
//                 let b_idx = 2 * channel_size + y * width + x;
//
//                 flat_data[r_idx] = pixel[2] as f32 / 255.0; // R
//                 flat_data[g_idx] = pixel[1] as f32 / 255.0; // G
//                 flat_data[b_idx] = pixel[0] as f32 / 255.0; // B
//             }
//         }
//     }
//
//     // Create the model input array from the flat data
//     let model_input = Array::from_shape_vec((1, 3, height, width), flat_data)?;
//
//     // Do whatever you need with model_input here
//     let duration = start_time.elapsed();
//
//     println!("{:?}", duration);
//     Ok(())
// }

pub fn detection_with_mat(session: &Session, frame: &Mat) -> Result<()> {
    let start_time = Instant::now();

    // Get frame dimensions
    let width = frame.cols() as usize;
    let height = frame.rows() as usize;
    let channels = frame.channels() as usize;

    // Check supported formats
    if channels != 3 && channels != 4 {
        anyhow::bail!("Unsupported image format. Expected 3 or 4 channels.");
    }

    // Ensure continuous memory for fast indexing
    if !frame.is_continuous() {
        anyhow::bail!("Expected continuous Mat data for parallel processing.");
    }

    // Raw byte buffer (BGR[A] interleaved)
    let data = frame.data_bytes()?;
    let channel_size = width * height;

    // Step 1: parallel convert each pixel chunk to [r, g, b]
    let triples: Vec<[f32; 3]> = data
        .par_chunks_exact(channels)
        .map(|pix| {
            let b = pix[0] as f32 / 255.0;
            let g = pix[1] as f32 / 255.0;
            let r = pix[2] as f32 / 255.0;
            [r, g, b]
        })
        .collect();

    // Step 2: flatten into planar (3, H, W)
    let mut flat_data = Vec::with_capacity(3 * channel_size);
    // push R channel
    flat_data.extend(triples.iter().map(|t| t[0]));
    // push G channel
    flat_data.extend(triples.iter().map(|t| t[1]));
    // push B channel
    flat_data.extend(triples.iter().map(|t| t[2]));

    // Construct ndarray input
    let model_input = Array::from_shape_vec((1, 3, height, width), flat_data)?;

    // TODO: use `session` and `model_input` for inference

    let duration = start_time.elapsed();
    println!("Detection time: {:?}", duration);

    Ok(())
}
