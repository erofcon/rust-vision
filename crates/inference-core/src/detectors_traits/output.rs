use crate::detectors::{Detectors, Output};
use crate::utils::{BoundingBox, intersection, union};
use anyhow::{Result, anyhow};
use ndarray::{Array, Axis, IxDyn, s};
use rayon::prelude::*;

impl Output for Detectors {
    fn process_yolo11s_output(
        output: Array<f32, IxDyn>,
        original_img_width: f32,
        original_img_height: f32,
        prob_threshold: Option<f32>,
        out_classes: Option<&[usize]>,
    ) -> Result<Vec<(BoundingBox, usize, f32)>> {
        let scale_x = original_img_width / 640f32;
        let scale_y = original_img_height / 640f32;

        let prob_threshold = prob_threshold.unwrap_or(0.45);
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
                    label: None,
                };
                Some((bbox, class_id, prob))
            })
            .collect();

        boxes.sort_unstable_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        let mut selected = Vec::with_capacity(boxes.len());
        while let Some(current) = boxes.pop() {
            selected.push(current);

            let current_bbox = &selected.last().unwrap().0;

            boxes.retain(|b| {
                let other_bbox = &b.0;
                let iou = intersection(current_bbox, other_bbox) / union(current_bbox, other_bbox);
                iou <= iou_threshold
            });
        }

        Ok(selected)
    }

    fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> Result<f32> {
        if vec1.len() != vec2.len() {
            return Err(anyhow!(
                "Vectors must be the same length.: {} != {}",
                vec1.len(),
                vec2.len()
            ));
        }

        if vec1.is_empty() {
            return Err(anyhow!("Vectors cannot be empty"));
        }

        let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();

        let norm1_sq: f32 = vec1.iter().map(|x| x * x).sum();
        let norm2_sq: f32 = vec2.iter().map(|x| x * x).sum();

        if norm1_sq == 0.0 || norm2_sq == 0.0 {
            return Ok(0.0);
        }

        let norm_product = (norm1_sq * norm2_sq).sqrt();

        if !norm_product.is_finite() || norm_product == 0.0 {
            return Ok(0.0);
        }

        let similarity = dot_product / norm_product;

        if !similarity.is_finite() {
            return Ok(0.0);
        }

        Ok(similarity.clamp(-1.0, 1.0))
    }
}
