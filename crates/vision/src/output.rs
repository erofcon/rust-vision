use crate::detection::{Detectors, Output};
use crate::utils::{BoundingBox, intersection, union};
use ndarray::{Array, Array4, Axis, IxDyn, s};
use ort::inputs;
use ort::session::Session;
use rayon::prelude::*;

impl Output for Detectors {
    fn process_yolo11s_output(
        output: Array<f32, IxDyn>,
        original_img_width: f32,
        original_img_height: f32,
        out_classes: Option<&[usize]>,
    ) -> anyhow::Result<Vec<(BoundingBox, usize, f32)>> {
        let scale_x = original_img_width / 640f32;
        let scale_y = original_img_height / 640f32;

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

    fn extract_face_embedding(
        session: &Session,
        input_tensor: Array4<f32>,
    ) -> anyhow::Result<Vec<f32>> {
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
}
