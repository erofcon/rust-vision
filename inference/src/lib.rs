pub mod utils;

use crate::utils::{intersection, union, YOLOV11_CLASS_LABELS};
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
    ) -> Result<Vec<(BoundingBox, f32)>> {
        let image = Self::prepare_image(frame)?;

        let input = inputs!["images"=>image]?;

        let outputs = self.session.run(input)?;

        let output = outputs["output0"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned();

        Self::process_output(output, original_img_width, original_img_height)
    }

    fn process_output(
        output: Array<f32, IxDyn>,
        original_img_width: usize,
        original_img_height: usize,
    ) -> Result<Vec<(BoundingBox, f32)>> {
        let mut boxes = Vec::new();
        let output = output.slice(s![.., .., 0]);

        for row in output.axis_iter(Axis(0)) {
            let row: Vec<_> = row.iter().copied().collect();
            let (class_id, prob) = row
                .iter()
                .skip(4)
                .enumerate()
                .map(|(index, value)| (index, *value))
                .reduce(|accum, row| if row.1 > accum.1 { row } else { accum })
                .unwrap();
            if prob < 0.35 {
                continue;
            }

            let xc = row[0] / 640. * (original_img_width as f32);
            let yc = row[1] / 640. * (original_img_height as f32);
            let w = row[2] / 640. * (original_img_width as f32);
            let h = row[3] / 640. * (original_img_height as f32);
            boxes.push((
                BoundingBox {
                    x1: xc - w / 2.,
                    y1: yc - h / 2.,
                    x2: xc + w / 2.,
                    y2: yc + h / 2.,
                },
                // class_id,
                prob,
            ));
        }

        boxes.sort_by(|box1, box2| box2.1.total_cmp(&box1.1));
        let mut result = Vec::new();

        while !boxes.is_empty() {
            result.push(boxes[0]);
            boxes = boxes
                .iter()
                .filter(|box1| {
                    intersection(&boxes[0].0, &box1.0) / union(&boxes[0].0, &box1.0) < 0.7
                })
                .copied()
                .collect();
        }

        Ok(result)
    }

    fn prepare_image(
        frame: &VideoFrame<gst_video::video_frame::Readable>,
    ) -> Result<Array<f32, ndarray::Dim<[usize; 4]>>> {
        let frame_width = frame.width() as usize;
        let frame_height = frame.height() as usize;

        let channel_size = frame_width * frame_height;
        let total_size = 1 * 3 * frame_height * frame_width;
        let mut flat_data = vec![0.0f32; total_size];

        let stride = frame.plane_stride()[0] as usize;
        let data = frame.plane_data(0)?;

        let r_offset = 0;
        let g_offset = channel_size;
        let b_offset = 2 * channel_size;

        for y in 0..frame_height {
            let row_offset = y * stride;
            let y_offset = y * frame_width;

            for x in 0..frame_width {
                let pixel_pos = row_offset + x * 3;
                let dest_pos = y_offset + x;

                if pixel_pos + 2 < data.len() {
                    flat_data[r_offset + dest_pos] = data[pixel_pos] as f32 / 255.0;
                    flat_data[g_offset + dest_pos] = data[pixel_pos + 1] as f32 / 255.0;
                    flat_data[b_offset + dest_pos] = data[pixel_pos + 2] as f32 / 255.0;
                }
            }
        }

        let input = Array::from_shape_vec((1, 3, frame_height, frame_width), flat_data)?;

        Ok(input)
    }
}
