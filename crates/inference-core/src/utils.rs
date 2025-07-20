use crate::detectors::{Detectors, Prepare};
use anyhow::Result;
use image::{DynamicImage, RgbImage};
use opencv::core::{Mat, Point, Scalar};
use opencv::imgproc;

const MAX_YAW_ANGLE_USABLE: f32 = 60.0; // Поворот влево-вправо (можно выровнять поворотом)
const MAX_PITCH_ANGLE_USABLE: f32 = 45.0; // Поворот вверх-вниз (критичный - нельзя исправить поворотом)
const MAX_ROLL_ANGLE_USABLE: f32 = 45.0; // Наклон головы (можно выровнять поворотом)

// Пороги для определения качественного фронтального лица (более строгие)
const MAX_YAW_ANGLE_FRONTAL: f32 = 15.0;
const MAX_PITCH_ANGLE_FRONTAL: f32 = 15.0;
const MAX_ROLL_ANGLE_FRONTAL: f32 = 15.0;

#[derive(Debug, Clone)]
pub struct HeadPose {
    pub yaw: f32,   // Поворот влево-вправо
    pub pitch: f32, // Поворот вверх-вниз
    pub roll: f32,  // Наклон головы
}

#[derive(Debug, Clone)]
pub enum FaceQuality {
    Excellent, // Идеальное фронтальное лицо
    Good,      // Хорошее лицо, требует выравнивания
    Poor,      // Плохое лицо (профиль, сильный наклон)
}

#[derive(Debug, Clone)]
pub struct Detection {
    pub bbox: [f32; 4],
    pub confidence: f32,
    pub keypoints: Vec<[f32; 3]>,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
}

#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub label: Option<String>,
    pub confidence: f32,
}

pub fn intersection(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    (box1.x2.min(box2.x2) - box1.x1.max(box2.x1)) * (box1.y2.min(box2.y2) - box1.y1.max(box2.y1))
}

pub fn union(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    ((box1.x2 - box1.x1) * (box1.y2 - box1.y1)) + ((box2.x2 - box2.x1) * (box2.y2 - box2.y1))
        - intersection(box1, box2)
}

pub fn img_to_rgb8(image: &DynamicImage, width: u32, height: u32) -> Result<RgbImage> {
    let image: DynamicImage = if image.width() != width || image.height() != height {
        Detectors::resize(image, width, height)?
    } else {
        image.clone()
    };

    let img_rgb: RgbImage = match image {
        DynamicImage::ImageRgb8(buf) => buf.clone(),
        _ => image.to_rgb8(),
    };

    Ok(img_rgb)
}

pub fn apply_nms(mut detections: Vec<Detection>) -> Result<Vec<Detection>> {
    if detections.is_empty() {
        return Ok(detections);
    }

    let mut keep = Vec::new();
    let mut suppressed = vec![false; detections.len()];

    for i in 0..detections.len() {
        if suppressed[i] {
            continue;
        }

        keep.push(detections[i].clone());

        for j in (i + 1)..detections.len() {
            if suppressed[j] {
                continue;
            }

            let iou = calculate_iou(&detections[i].bbox, &detections[j].bbox);
            if iou > 0.4 {
                suppressed[j] = true;
            }
        }
    }

    Ok(keep)
}

pub fn calculate_iou(bbox1: &[f32; 4], bbox2: &[f32; 4]) -> f32 {
    let x1 = bbox1[0].max(bbox2[0]);
    let y1 = bbox1[1].max(bbox2[1]);
    let x2 = bbox1[2].min(bbox2[2]);
    let y2 = bbox1[3].min(bbox2[3]);

    if x2 <= x1 || y2 <= y1 {
        return 0.0;
    }

    let intersection = (x2 - x1) * (y2 - y1);
    let area1 = (bbox1[2] - bbox1[0]) * (bbox1[3] - bbox1[1]);
    let area2 = (bbox2[2] - bbox2[0]) * (bbox2[3] - bbox2[1]);
    let union = area1 + area2 - intersection;

    if union <= 0.0 {
        return 0.0;
    }

    intersection / union
}

pub fn calculate_head_pose(keypoints: &[[f32; 3]]) -> (f32, f32, f32) {
    if keypoints.len() < 5
        || keypoints[0][2] < 0.3
        || keypoints[1][2] < 0.3
        || keypoints[2][2] < 0.3
    {
        return (0.0, 0.0, 0.0);
    }

    let [left_eye, right_eye, nose, _left_mouth, _right_mouth] = [
        keypoints[0],
        keypoints[1],
        keypoints[2],
        keypoints[3],
        keypoints[4],
    ];

    // YAW = горизонтальное смещение носа
    let eye_center_x = (left_eye[0] + right_eye[0]) / 2.0;
    let nose_offset_x = nose[0] - eye_center_x;
    let yaw = (nose_offset_x * 0.5).clamp(-90.0, 90.0);

    // PITCH = вертикальное смещение носа
    let eye_center_y = (left_eye[1] + right_eye[1]) / 2.0;
    let nose_offset_y = nose[1] - eye_center_y;
    let pitch = (nose_offset_y * 0.3).clamp(-45.0, 45.0);

    // ROLL = угол наклона линии глаз
    let dx = right_eye[0] - left_eye[0];
    let dy = right_eye[1] - left_eye[1];
    let roll = (dy.atan2(dx) * 180.0 / std::f32::consts::PI).clamp(-45.0, 45.0);

    (yaw, pitch, roll)
}

pub fn draw_detections(img: &mut Mat, detections: &[Detection]) -> Result<()> {
    for (idx, detection) in detections.iter().enumerate() {
        // Зеленый для фронтальных, красный для профильных
        let color = Scalar::new(0.0, 255.0, 0.0, 0.0); // Зеленый

        // Рамка
        let pt1 = Point::new(detection.bbox[0] as i32, detection.bbox[1] as i32);
        let pt2 = Point::new(detection.bbox[2] as i32, detection.bbox[3] as i32);
        imgproc::rectangle(
            img,
            opencv::core::Rect::from_points(pt1, pt2),
            color,
            2,
            imgproc::LINE_8,
            0,
        )?;

        // Подпись
        let label = format!(
            "{}: {:.2} | {} | Y:°",
            idx + 1,
            detection.confidence,
            detection.yaw
        );

        imgproc::put_text(
            img,
            &label,
            Point::new(detection.bbox[0] as i32, detection.bbox[1] as i32 - 10),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            color,
            1,
            imgproc::LINE_8,
            false,
        )?;
    }
    Ok(())
}


pub fn bbox_to_array(bbox: &BoundingBox) -> [f32; 4] {
    [bbox.x1, bbox.y1, bbox.x2, bbox.y2]
}