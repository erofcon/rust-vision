pub mod utils;

use crate::utils::{intersection, union};
use anyhow::Result;
use gst_video::{VideoFrame, VideoFrameExt};
use ndarray::{s, Array, Axis, IxDyn};
use ort::execution_providers::CUDAExecutionProvider;
use ort::inputs;
use ort::session::Session;
use rayon::prelude::*;
use std::sync::Arc;
use utils::BoundingBox;

pub struct Inference {
    session: Arc<Session>,
}

impl Inference {
    pub fn new(model_path: &str) -> Self {
        ort::init()
            .with_execution_providers([CUDAExecutionProvider::default().build().error_on_failure()])
            .commit()
            .unwrap();

        let session = Arc::new(
            Session::builder()
                .unwrap()
                .with_inter_threads(4)
                .unwrap()
                .commit_from_file(model_path)
                .unwrap(),
        );

        println!("Initializing model from: {}", model_path);

        Self { session }
    }

    pub fn inference(
        &self,
        frame: &VideoFrame<gst_video::video_frame::Readable>,
        original_img_width: usize,
        original_img_height: usize,
    ) -> Result<Vec<(BoundingBox, usize, f32)>> {

        let image = Self::prepare_image(frame)?;

        let input = inputs!["images"=>image]?;

        let outputs = self.session.run(input)?;

        let output = outputs["output0"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned();

        let result = Self::process_output(output, original_img_width, original_img_height);

        result
    }

    fn process_output(
        output: Array<f32, IxDyn>,
        original_img_width: usize,
        original_img_height: usize,
    ) -> Result<Vec<(BoundingBox, usize, f32)>> {
        let scale_x = original_img_width as f32 / 640.0;
        let scale_y = original_img_height as f32 / 640.0;
        let prob_threshold = 0.35;
        let iou_threshold = 0.7;

        let output = output.slice(s![.., .., 0]);

        let mut boxes: Vec<(BoundingBox, usize, f32)> = output
            .axis_iter(Axis(0))
            .into_par_iter()
            .filter_map(|row| {
                row.iter()
                    .skip(4)
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                    .and_then(|(class_id, &prob)| {
                        if prob < prob_threshold {
                            None
                        } else {
                            let xc = row[0_usize] * scale_x;
                            let yc = row[1_usize] * scale_y;
                            let w = row[2_usize] * scale_x;
                            let h = row[3_usize] * scale_y;
                            Some((
                                BoundingBox {
                                    x1: xc - w / 2.0,
                                    y1: yc - h / 2.0,
                                    x2: xc + w / 2.0,
                                    y2: yc + h / 2.0,
                                },
                                class_id,
                                prob,
                            ))
                        }
                    })
            })
            .collect();

        boxes.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        let mut selected = Vec::with_capacity(boxes.len());
        boxes.reverse();

        while let Some(current) = boxes.pop() {
            selected.push(current.clone());

            boxes.retain(|b| {
                let iou = intersection(&current.0, &b.0) / union(&current.0, &b.0);
                iou <= iou_threshold
            });
        }

        Ok(selected)
    }

    fn prepare_image(
        frame: &VideoFrame<gst_video::video_frame::Readable>,
    ) -> Result<Array<f32, ndarray::Dim<[usize; 4]>>> {
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
                    // Защищаемся от выхода за границы data
                    if pixel_pos + 2 < data.len() {
                        r_row[x] = data[pixel_pos] as f32 / 255.0;
                        g_row[x] = data[pixel_pos + 1] as f32 / 255.0;
                        b_row[x] = data[pixel_pos + 2] as f32 / 255.0;
                    }
                }
            });

        let input = Array::from_shape_vec((1, 3, frame_height, frame_width), flat_data)?;
        Ok(input)
    }
}
