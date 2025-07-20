use crate::detectors::{Detectors, Output};
use crate::utils::{BoundingBox, Detection, FaceQuality, apply_nms, calculate_head_pose, calculate_iou, intersection, union, bbox_to_array};
use anyhow::{Result, anyhow};
use ndarray::{Array, Axis, IxDyn, s};
use rayon::prelude::*;

impl Output for Detectors {
    fn process_yolo11s_output(
        output: Array<f32, IxDyn>,
        original_img_width: f32,
        original_img_height: f32,
        input_width: f32,
        input_height: f32,
        scale: f32,
        pad_x: f32,
        pad_y: f32,
        prob_threshold: Option<f32>,
        out_classes: Option<&[usize]>,
    ) -> Result<Vec<BoundingBox>, Box<dyn std::error::Error>>  {
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

                // Координаты относительно input размера (640x640)
                let xc = row[0_usize];
                let yc = row[1_usize];
                let w = row[2_usize];
                let h = row[3_usize];

                // Корректируем с учетом padding
                let corrected_xc = (xc - pad_x) / scale;
                let corrected_yc = (yc - pad_y) / scale;
                let corrected_w = w / scale;
                let corrected_h = h / scale;

                let bbox = BoundingBox {
                    x1: corrected_xc - corrected_w / 2.0,
                    y1: corrected_yc - corrected_h / 2.0,
                    x2: corrected_xc + corrected_w / 2.0,
                    y2: corrected_yc + corrected_h / 2.0,
                    label: None,
                    confidence: prob,
                };
                Some(bbox)
            })
            .collect();

        // Сортируем по уверенности (по убыванию)
        boxes.sort_unstable_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        // Non-Maximum Suppression (NMS)
        let mut selected = Vec::with_capacity(boxes.len());
        let mut suppressed = vec![false; boxes.len()];

        for (i, current_box) in boxes.iter().enumerate() {
            if suppressed[i] {
                continue;
            }

            selected.push(current_box.clone());

            // Подавляем все остальные box'ы с высоким IoU
            for (j, other_box) in boxes.iter().enumerate().skip(i + 1) {
                if suppressed[j] {
                    continue;
                }

                let iou = calculate_iou(&bbox_to_array(current_box), &bbox_to_array(other_box));
                if iou > iou_threshold {
                    suppressed[j] = true;
                }
            }
        }

        Ok(selected)
    }

    fn cosine_similarity(vec1: &[f32], vec2: &[f32]) -> Result<f32> {
        let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
        let norm1: f32 = vec1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm2: f32 = vec2.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm1 == 0.0 || norm2 == 0.0 {
            return Ok(0.0);
        }

        Ok((dot_product / (norm1 * norm2)).clamp(-1.0, 1.0))
    }

    fn process_detections(
        outputs: &Array<f32, IxDyn>,
        original_width: i32,
        original_height: i32,
        scale: f32,
        pad_x: f32,
        pad_y: f32,
    ) -> Result<Vec<Detection>> {
        let detections = Vec::new();
        let shape = outputs.shape();

        if shape.len() != 3 {
            return Ok(detections);
        }

        let (_, num_features, num_detections) = (shape[0], shape[1], shape[2]);
        let mut candidates = Vec::new();

        for i in 0..num_detections {
            let confidence = outputs[[0, 4, i]];
            // TODO: add confidence to config

            if confidence <= 0.7 {
                continue;
            }

            let cx = outputs[[0, 0, i]];
            let cy = outputs[[0, 1, i]];
            let w = outputs[[0, 2, i]];
            let h = outputs[[0, 3, i]];

            let x_center = (cx - pad_x) / scale;
            let y_center = (cy - pad_y) / scale;
            let width = w / scale;
            let height = h / scale;

            let x1 = x_center - width / 2.0;
            let y1 = y_center - height / 2.0;
            let x2 = x_center + width / 2.0;
            let y2 = y_center + height / 2.0;

            if x1 >= 0.0
                && y1 >= 0.0
                && x2 <= original_width as f32
                && y2 <= original_height as f32
                && x2 > x1
                && y2 > y1
                && width > 10.0
                && height > 10.0
            {
                let bbox = [x1, y1, x2, y2];
                let mut keypoints = Vec::new();

                // Извлекаем ключевые точки
                for j in 0..5 {
                    let keypoint_base = 5 + j * 3;
                    if keypoint_base + 2 < num_features {
                        let kp_x = (outputs[[0, keypoint_base, i]] - pad_x) / scale;
                        let kp_y = (outputs[[0, keypoint_base + 1, i]] - pad_y) / scale;
                        let kp_visibility = outputs[[0, keypoint_base + 2, i]];
                        keypoints.push([kp_x, kp_y, kp_visibility]);
                    }
                }

                let (yaw, pitch, roll) = calculate_head_pose(&keypoints);

                candidates.push(Detection {
                    bbox,
                    confidence,
                    keypoints,
                    yaw,
                    pitch,
                    roll,
                });
            }
        }

        // NMS
        candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        apply_nms(candidates)
    }
}
