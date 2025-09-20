use anyhow::Result;
use ndarray::{Array, ArrayBase, Dim, Ix, IxDyn, OwnedRepr};
use opencv::core::{Mat, MatTrait, MatTraitConst, Point, Scalar, Size};
use opencv::imgproc;
use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider};
use ort::inputs;
use ort::session::{Session, SessionOutputs};

const MODEL_WH: i32 = 800;
const CONFIDENCE_THRESHOLD: f32 = 0.45;
const NMS_THRESHOLD: f32 = 0.45;

// Строгие пороги для фронтального лица
const MAX_YAW_FRONTAL: f32 = 25.0;   // Поворот влево-вправо
const MAX_PITCH_FRONTAL: f32 = 20.0; // Поворот вверх-вниз
const MAX_ROLL_FRONTAL: f32 = 90.0;  // Наклон головы

#[derive(Debug, Clone)]
struct Detection {
    bbox: [f32; 4],
    confidence: f32,
    keypoints: Vec<[f32; 3]>,
    is_frontal: bool,
    yaw: f32,
    pitch: f32,
    roll: f32,
}

fn load_model(detection_model_path: &str) -> Result<Session> {
    ort::init()
        .with_execution_providers([
            CUDAExecutionProvider::default().build().error_on_failure(),
            CPUExecutionProvider::default().build().error_on_failure(),
        ])
        .commit()?;
    Ok(Session::builder()?
        .with_inter_threads(4)?
        .commit_from_file(detection_model_path)?)
}

fn prepare_img(img: &Mat) -> Result<ArrayBase<OwnedRepr<f32>, Dim<[Ix; 4]>>> {
    let mut model_input = Array::zeros((1, 3, MODEL_WH as usize, MODEL_WH as usize));

    for y in 0..MODEL_WH {
        for x in 0..MODEL_WH {
            let pixel = img.at_2d::<opencv::core::Vec3b>(y, x)?;
            model_input[[0, 0, y as usize, x as usize]] = pixel[2] as f32 / 255.0;
            model_input[[0, 1, y as usize, x as usize]] = pixel[1] as f32 / 255.0;
            model_input[[0, 2, y as usize, x as usize]] = pixel[0] as f32 / 255.0;
        }
    }
    Ok(model_input)
}

fn inference(session: Session, model_input: ArrayBase<OwnedRepr<f32>, Dim<[Ix; 4]>>) -> Result<Array<f32, IxDyn>> {
    let outputs: SessionOutputs = session.run(inputs!["images" => model_input]?)?;
    let output = outputs["output0"].try_extract_tensor::<f32>()?.into_owned();
    Ok(output)
}

fn resize_with_padding(src: &Mat) -> Result<(Mat, f32, f32, f32)> {
    let src_h = src.rows() as f32;
    let src_w = src.cols() as f32;
    let target_size = MODEL_WH as f32;
    let scale = (target_size / src_w).min(target_size / src_h);

    let new_w = (src_w * scale) as i32;
    let new_h = (src_h * scale) as i32;

    let mut resized = Mat::default();
    opencv::imgproc::resize(src, &mut resized, Size::new(new_w, new_h), 0.0, 0.0, opencv::imgproc::INTER_LINEAR)?;

    let mut padded = Mat::default();
    opencv::core::copy_make_border(
        &resized, &mut padded,
        (MODEL_WH - new_h) / 2, (MODEL_WH - new_h) / 2,
        (MODEL_WH - new_w) / 2, (MODEL_WH - new_w) / 2,
        opencv::core::BORDER_CONSTANT,
        Scalar::new(114.0, 114.0, 114.0, 0.0),
    )?;

    let pad_x = ((MODEL_WH - new_w) / 2) as f32;
    let pad_y = ((MODEL_WH - new_h) / 2) as f32;

    Ok((padded, scale, pad_x, pad_y))
}

fn calculate_head_pose(keypoints: &[[f32; 3]]) -> (f32, f32, f32) {
    if keypoints.len() < 5 {
        return (0.0, 0.0, 0.0);
    }

    let left_eye = &keypoints[0];
    let right_eye = &keypoints[1];
    let nose = &keypoints[2];
    let left_mouth = &keypoints[3];
    let right_mouth = &keypoints[4];

    // Проверяем видимость ключевых точек
    if left_eye[2] < 0.3 || right_eye[2] < 0.3 || nose[2] < 0.3 {
        return (0.0, 0.0, 0.0);
    }

    // Roll - наклон головы по линии глаз
    let eye_dx = right_eye[0] - left_eye[0];
    let eye_dy = right_eye[1] - left_eye[1];
    let roll = eye_dy.atan2(eye_dx) * 180.0 / std::f32::consts::PI;

    // Yaw - поворот влево/вправо по смещению носа
    let eye_center_x = (left_eye[0] + right_eye[0]) / 2.0;
    let eye_distance = ((right_eye[0] - left_eye[0]).powi(2) + (right_eye[1] - left_eye[1]).powi(2)).sqrt();

    // ИСПРАВЛЕНО: более точный расчет yaw через относительную видимость глаз
    let left_eye_visibility = left_eye[2];
    let right_eye_visibility = right_eye[2];
    let visibility_diff = (right_eye_visibility - left_eye_visibility).abs();

    // Если один глаз сильно менее виден - это профиль
    let nose_offset = (nose[0] - eye_center_x) / eye_distance;
    let yaw = if visibility_diff > 0.3 {
        // Профиль - усиливаем значение yaw
        if left_eye_visibility < right_eye_visibility {
            -60.0 + nose_offset * 30.0 // Левый профиль
        } else {
            60.0 + nose_offset * 30.0  // Правый профиль
        }
    } else {
        nose_offset * 45.0 // Обычный расчет для фронтальных лиц
    };

    // Pitch - поворот вверх/вниз
    let eye_center_y = (left_eye[1] + right_eye[1]) / 2.0;
    let pitch = if left_mouth[2] > 0.3 && right_mouth[2] > 0.3 {
        let mouth_center_y = (left_mouth[1] + right_mouth[1]) / 2.0;
        let face_height = mouth_center_y - eye_center_y;
        let expected_face_height = eye_distance * 1.2;
        let pitch_ratio = (face_height / expected_face_height - 1.0) * 45.0;
        pitch_ratio.clamp(-45.0, 45.0)
    } else {
        let nose_vertical_offset = (nose[1] - eye_center_y) / eye_distance;
        nose_vertical_offset * 30.0
    };

    (
        yaw.clamp(-90.0, 90.0),
        pitch.clamp(-45.0, 45.0),
        roll.clamp(-45.0, 45.0)
    )
}

fn is_frontal_face(yaw: f32, pitch: f32, roll: f32) -> bool {
    yaw.abs() <= MAX_YAW_FRONTAL &&
        pitch.abs() <= MAX_PITCH_FRONTAL &&
        roll.abs() <= MAX_ROLL_FRONTAL
}

fn process_detections(
    outputs: &Array<f32, IxDyn>,
    original_width: i32,
    original_height: i32,
    scale: f32,
    pad_x: f32,
    pad_y: f32,
) -> Result<Vec<Detection>> {
    let mut detections = Vec::new();
    let shape = outputs.shape();

    if shape.len() != 3 { return Ok(detections); }

    let (_, num_features, num_detections) = (shape[0], shape[1], shape[2]);
    let mut candidates = Vec::new();

    for i in 0..num_detections {
        let confidence = outputs[[0, 4, i]];
        if confidence <= CONFIDENCE_THRESHOLD { continue; }

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

        if x1 >= 0.0 && y1 >= 0.0 && x2 <= original_width as f32 && y2 <= original_height as f32
            && x2 > x1 && y2 > y1 && width > 10.0 && height > 10.0 {

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
            let is_frontal = is_frontal_face(yaw, pitch, roll);

            candidates.push(Detection {
                bbox,
                confidence,
                keypoints,
                is_frontal,
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

fn apply_nms(detections: Vec<Detection>) -> Result<Vec<Detection>> {
    let mut keep = Vec::new();
    let mut suppressed = vec![false; detections.len()];

    for i in 0..detections.len() {
        if suppressed[i] { continue; }
        keep.push(detections[i].clone());

        for j in (i + 1)..detections.len() {
            if !suppressed[j] && calculate_iou(&detections[i].bbox, &detections[j].bbox) > NMS_THRESHOLD {
                suppressed[j] = true;
            }
        }
    }
    Ok(keep)
}

fn calculate_iou(bbox1: &[f32; 4], bbox2: &[f32; 4]) -> f32 {
    let x1 = bbox1[0].max(bbox2[0]);
    let y1 = bbox1[1].max(bbox2[1]);
    let x2 = bbox1[2].min(bbox2[2]);
    let y2 = bbox1[3].min(bbox2[3]);

    if x2 <= x1 || y2 <= y1 { return 0.0; }

    let intersection = (x2 - x1) * (y2 - y1);
    let area1 = (bbox1[2] - bbox1[0]) * (bbox1[3] - bbox1[1]);
    let area2 = (bbox2[2] - bbox2[0]) * (bbox2[3] - bbox2[1]);
    let union = area1 + area2 - intersection;

    if union <= 0.0 { 0.0 } else { intersection / union }
}

fn draw_detections(img: &mut Mat, detections: &[Detection]) -> Result<()> {
    for (idx, detection) in detections.iter().enumerate() {
        // Зеленый для фронтальных, красный для профильных
        let color = if detection.is_frontal {
            Scalar::new(0.0, 255.0, 0.0, 0.0)  // Зеленый
        } else {
            Scalar::new(0.0, 0.0, 255.0, 0.0)  // Красный
        };

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
            "{}: {:.2} | {} | Y:{:.1}°",
            idx + 1,
            detection.confidence,
            if detection.is_frontal { "FRONT" } else { "PROFILE" },
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

fn main() -> Result<()> {
    let model_path = "models/yolo11s-pose_widerface.onnx";
    let img_path = "assets/test/img_2.png";

    let img = opencv::imgcodecs::imread(img_path, opencv::imgcodecs::IMREAD_COLOR)?;
    let session = load_model(model_path)?;

    let original_width = img.cols();
    let original_height = img.rows();

    let (resized_img, scale, pad_x, pad_y) = resize_with_padding(&img)?;
    let prepared_img = prepare_img(&resized_img)?;
    let session_outputs = inference(session, prepared_img)?;

    let detections = process_detections(
        &session_outputs,
        original_width,
        original_height,
        scale,
        pad_x,
        pad_y,
    )?;

    // Результаты
    let frontal_count = detections.iter().filter(|d| d.is_frontal).count();
    let profile_count = detections.len() - frontal_count;

    println!("=== РЕЗУЛЬТАТЫ ===");
    println!("Всего лиц: {}", detections.len());
    println!("Фронтальных: {}", frontal_count);
    println!("Профильных: {}", profile_count);

    for (i, detection) in detections.iter().enumerate() {
        println!(
            "Лицо {}: {} | Conf: {:.3} | Yaw: {:.1}° | Pitch: {:.1}° | Roll: {:.1}°",
            i + 1,
            if detection.is_frontal { "ФРОНТ" } else { "ПРОФИЛЬ" },
            detection.confidence,
            detection.yaw,
            detection.pitch,
            detection.roll
        );
    }

    let mut result_img = img.clone();
    draw_detections(&mut result_img, &detections)?;

    opencv::highgui::imshow("Face Detection: Green=Frontal, Red=Profile", &result_img)?;
    opencv::highgui::wait_key(0)?;

    Ok(())
}