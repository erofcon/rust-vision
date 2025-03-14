use anyhow::Result;
use gst_video::{VideoFrame, VideoFrameExt};
use ndarray::{s, Array, ArrayView, Axis, IxDyn};
use ort::execution_providers::CUDAExecutionProvider;
use ort::inputs;
use ort::session::Session;
use rayon::prelude::*;

use std::sync::Arc;

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
    ) -> Result<()> {
        let image = Self::prepare_image(frame, original_img_width, original_img_height).unwrap();

        let input = inputs!["images"=>image]?;

        let outputs = self.session.run(input)?;

        let output = outputs["output0"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned();

        Self::process_output(output, original_img_width, original_img_height);

        // let mut boxes = Vec::new();
        // let output = output.slice(s![.., .., 0]);
        // for row in output.axis_iter(Axis(0)) {
        //     let row: Vec<_> = row.iter().copied().collect();
        //     let (class_id, prob) = row
        //         .iter()
        //         .skip(4)
        //         .enumerate()
        //         .map(|(index, value)| (index, *value))
        //         .reduce(|accum, row| if row.1 > accum.1 { row } else { accum })
        //         .unwrap();
        //     if prob < 0.5 {
        //         continue;
        //     }
        //     let xc = row[0] / 640. * (original_img_width as f32);
        //     let yc = row[1] / 640. * (original_img_height as f32);
        //     let w = row[2] / 640. * (original_img_width as f32);
        //     let h = row[3] / 640. * (original_img_height as f32);
        //     boxes.push((
        //         BoundingBox {
        //             x1: xc - w / 2.,
        //             y1: yc - h / 2.,
        //             x2: xc + w / 2.,
        //             y2: yc + h / 2.
        //         },
        //         label,
        //         prob
        //     ));
        //
        //     println!("{}", prob)
        // }

        Ok(())
    }

    fn process_output(
        output: Array<f32, IxDyn>,
        original_img_width: usize,
        original_img_height: usize,
    ) -> Result<()> {
        let output = output.slice(s![.., .., 0]);

        // let processed_boxes: Vec<_> = output
        //     .axis_iter(Axis(0))
        //     .par_bridge()
        //     .filter_map(|row| {
        //         for row in output.axis_iter(Axis(0)) {
        //             let row: Vec<_> = row.iter().copied().collect();
        //             let (class_id, prob) = row
        //                 .iter()
        //                 .skip(4)
        //                 .enumerate()
        //                 .map(|(index, value)| (index, *value))
        //                 .reduce(|accum, row| if row.1 > accum.1 { row } else { accum })
        //                 .unwrap();
        //             if prob < 0.5 {
        //                 continue;
        //             }
        //             let xc = row[0] / 640. * (original_img_width as f32);
        //             let yc = row[1] / 640. * (original_img_height as f32);
        //             let w = row[2] / 640. * (original_img_width as f32);
        //             let h = row[3] / 640. * (original_img_height as f32);
        //             // boxes.push((
        //             //     BoundingBox {
        //             //         x1: xc - w / 2.,
        //             //         y1: yc - h / 2.,
        //             //         x2: xc + w / 2.,
        //             //         y2: yc + h / 2.
        //             //     },
        //             //     label,
        //             //     prob
        //             // ));
        //
        //             println!("{}", prob)
        //         }
        //
        //         Some(())
        //     })
        //     .collect();

        Ok(())
    }
    // fn run(&self, input: Array<f32, ndarray::Dim<[usize; 4]>>) -> Result<ArrayView<f32, IxDyn>> {
    //     // let input = ort::inputs!["images"=>input]?;
    //     // // let outputs = self.session.run(input)?;
    //     // //
    //     // // // let output = outputs["output0"]
    //     // // //     .try_extract_tensor::<f32>()?
    //     // // //     .t()
    //     // // //     .into_owned();
    //     // //
    //     // // let output = outputs["output0"].try_extract_tensor()?;
    //     //
    //     // let outputs = self.session.run(input)?;
    //     // let outputs = outputs[0].try_extract_tensor()?;
    //     //
    //     // Ok(outputs)
    // }

    fn prepare_image(
        frame: &VideoFrame<gst_video::video_frame::Readable>,
        width: usize,
        height: usize,
    ) -> Result<Array<f32, ndarray::Dim<[usize; 4]>>> {
        let frame_width = frame.width() as usize;
        let frame_height = frame.height() as usize;

        if frame_width != width || frame_height != height {
            anyhow::bail!("Size mismatch");
        }

        let channel_size = width * height;
        let total_size = 1 * 3 * height * width;
        let mut flat_data = vec![0.0f32; total_size];

        let stride = frame.plane_stride()[0] as usize;
        let data = frame.plane_data(0)?;

        let r_offset = 0;
        let g_offset = channel_size;
        let b_offset = 2 * channel_size;

        for y in 0..height {
            let row_offset = y * stride;
            let y_offset = y * width;

            for x in 0..width {
                let pixel_pos = row_offset + x * 3;
                let dest_pos = y_offset + x;

                if pixel_pos + 2 < data.len() {
                    flat_data[r_offset + dest_pos] = data[pixel_pos] as f32 / 255.0;
                    flat_data[g_offset + dest_pos] = data[pixel_pos + 1] as f32 / 255.0;
                    flat_data[b_offset + dest_pos] = data[pixel_pos + 2] as f32 / 255.0;
                }
            }
        }

        let input = Array::from_shape_vec((1, 3, height, width), flat_data)?;

        Ok(input)
    }
    fn parse_prediction(output: ArrayView<f32, IxDyn>) -> Result<()> {
        for pred in output.axis_iter(Axis(1)) {
            // Separate bbox and class values.
            // First 4 values correspond to bbox cx, cy, w, h
            const BBOX_OFFSET: usize = 4;
            let bbox = pred.slice(s![0..BBOX_OFFSET]);
            let clss = pred.slice(s![BBOX_OFFSET..BBOX_OFFSET + 80 as usize]);

            // Determine top1 class and its confidence.
            let mut max_class_id = 0;
            let mut max_confidence = 0f32;
            for (idx, cls_conf) in clss.into_iter().enumerate() {
                if cls_conf > &max_confidence {
                    max_confidence = *cls_conf;
                    max_class_id = idx;
                }
            }

            println!("Max class: {}", max_class_id);
        }

        // let preds: ArrayView<f32, Dim<[usize; 2]>> = output.slice(s![0, .., ..]);
        //
        //
        // for pred in preds.axis_iter(Axis(1)) {
        //     // Separate bbox and class values.
        //     // First 4 values correspond to bbox cx, cy, w, h
        //     const BBOX_OFFSET: usize = 4;
        //     let bbox = pred.slice(s![0..BBOX_OFFSET]);
        //     let clss = pred.slice(s![BBOX_OFFSET..BBOX_OFFSET + 80 as usize]);
        //
        //
        //     // Determine top1 class and its confidence.
        //     // let mut max_class_id = 0;
        //     // let mut max_confidence = 0f32;
        //     for (idx, cls_conf) in clss.into_iter().enumerate() {
        //
        //         println!("Cls[{}] = {}", idx, cls_conf);
        //
        //         // if cls_conf > &max_confidence {
        //         //     max_confidence = *cls_conf;
        //         //     max_class_id = idx;
        //         // }
        //     }
        //     //
        //     // if max_confidence < 0.24 {
        //     //     continue;
        //     // }
        //
        //     // println!("Max Confidence: {}", max_confidence);
        // }

        Ok(())
    }
}
