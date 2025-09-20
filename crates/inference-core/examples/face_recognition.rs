use anyhow::{Result, anyhow};
use image::{DynamicImage, Rgb};
use imageproc::drawing::{Canvas, draw_hollow_rect_mut};
use imageproc::rect::Rect;
use ndarray::parallel::prelude::IntoParallelIterator;
use ndarray::{Array, Array4, Axis, IxDyn, s};
use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider};
use ort::inputs;
use ort::session::Session;
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl BoundingBox {
    pub fn width(&self) -> f32 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> f32 {
        self.y2 - self.y1
    }

    pub fn to_rect(&self) -> Rect {
        Rect::at(self.x1 as i32, self.y1 as i32).of_size(self.width() as u32, self.height() as u32)
    }
}

pub fn intersection(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    (box1.x2.min(box2.x2) - box1.x1.max(box2.x1)) * (box1.y2.min(box2.y2) - box1.y1.max(box2.y1))
}

pub fn union(box1: &BoundingBox, box2: &BoundingBox) -> f32 {
    ((box1.x2 - box1.x1) * (box1.y2 - box1.y1)) + ((box2.x2 - box2.x1) * (box2.y2 - box2.y1))
        - intersection(box1, box2)
}

#[derive(Debug, Clone)]
pub struct FaceEmbedding {
    pub embedding: Vec<f32>,
}

pub struct FaceRecognitionSystem {
    detection_session: Session,
    recognition_session: Session,
}

impl FaceRecognitionSystem {
    pub async fn new(detection_model_path: &str, recognition_model_path: &str) -> Result<Self> {
        ort::init()
            .with_execution_providers([
                CUDAExecutionProvider::default().build().error_on_failure(),
                CPUExecutionProvider::default().build().error_on_failure(),
            ])
            .commit()?;

        let detection_session = Session::builder()?
            .with_inter_threads(4)?
            .commit_from_file(detection_model_path)?;

        let recognition_session = Session::builder()?
            .with_inter_threads(4)?
            .commit_from_file(recognition_model_path)?;

        Ok(Self {
            detection_session,
            recognition_session,
        })
    }

    pub async fn detect_faces(
        &self,
        image: &DynamicImage,
    ) -> Result<Vec<(BoundingBox, usize, f32)>> {
        let input_tensor = self.preprocess_image_for_detection(image)?;
        let input = inputs!["images"=>input_tensor]?;

        // let input = inputs!["images"=>Value::from_array(input_tensor)?]?;

        let outputs = self.detection_session.run(input)?;
        // .run(vec![Value::from_array(input_tensor)?])?;

        let output = outputs["output0"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned();

        // println!("{:?}", output.len());

        let results = self.process_output(
            output,
            image.width() as i32,
            image.height() as i32,
            640,
            640,
            None,
        )?;

        println!("Detected {} faces", results.len());

        Ok(results)
    }

    fn preprocess_image_for_detection(&self, image: &DynamicImage) -> Result<Array4<f32>> {
        let img = image.resize_exact(640, 640, image::imageops::FilterType::Lanczos3);
        let img_rgb = img.to_rgb8();

        let mut input = Array4::<f32>::zeros((1, 3, 640, 640));

        for (y, row) in img_rgb.rows().enumerate() {
            for (x, pixel) in row.enumerate() {
                let [r, g, b] = pixel.0;
                input[[0, 0, y, x]] = r as f32 / 255.0;
                input[[0, 1, y, x]] = g as f32 / 255.0;
                input[[0, 2, y, x]] = b as f32 / 255.0;
            }
        }

        Ok(input)
    }

    fn process_output(
        &self,
        output: Array<f32, IxDyn>,
        original_img_width: i32,
        original_img_height: i32,
        model_input_width: i32,
        model_input_height: i32,
        out_classes: Option<&[usize]>,
    ) -> Result<Vec<(BoundingBox, usize, f32)>> {
        let scale_x = original_img_width as f32 / model_input_width as f32;
        let scale_y = original_img_height as f32 / model_input_height as f32;
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

    pub fn draw_bounding_boxes(
        &self,
        image: &DynamicImage,
        detections: &[(BoundingBox, usize, f32)],
        output_path: &str,
    ) -> Result<()> {
        let mut img = image.to_rgb8();

        // Определяем цвета для рамок
        let colors = [
            Rgb([255, 0, 0]),   // Красный
            Rgb([0, 255, 0]),   // Зеленый
            Rgb([0, 0, 255]),   // Синий
            Rgb([255, 255, 0]), // Желтый
            Rgb([255, 0, 255]), // Фиолетовый
            Rgb([0, 255, 255]), // Голубой
        ];

        for (i, (bbox, class_id, confidence)) in detections.iter().enumerate() {
            let color = colors[i % colors.len()];
            let rect = bbox.to_rect();

            // Рисуем рамку (толщина 3 пикселя)
            for thickness in 0..3 {
                let thick_rect = Rect::at(
                    (rect.left() - thickness).max(0),
                    (rect.top() - thickness).max(0),
                )
                    .of_size(
                        (rect.width() + 2 * thickness as u32).min(img.width()),
                        (rect.height() + 2 * thickness as u32).min(img.height()),
                    );
                draw_hollow_rect_mut(&mut img, thick_rect, color);
            }

            println!(
                "Face {}: Box({:.1}, {:.1}, {:.1}, {:.1}), Confidence: {:.3}",
                i + 1,
                bbox.x1,
                bbox.y1,
                bbox.x2,
                bbox.y2,
                confidence
            );
        }

        // Сохраняем изображение
        img.save(output_path)?;
        println!("Изображение с рамками сохранено: {}", output_path);

        Ok(())
    }

    pub fn save_cropped_faces(
        &self,
        image: &DynamicImage,
        detections: &[(BoundingBox, usize, f32)],
        output_dir: &str,
    ) -> Result<()> {
        std::fs::create_dir_all(output_dir)?;

        for (i, (bbox, _, confidence)) in detections.iter().enumerate() {
            let padding = 0.0;
            let x1 = (bbox.x1 - padding).max(0.0) as u32;
            let y1 = (bbox.y1 - padding).max(0.0) as u32;
            let x2 = (bbox.x2 + padding).min(image.width() as f32) as u32;
            let y2 = (bbox.y2 + padding).min(image.height() as f32) as u32;

            let cropped = image.crop_imm(x1, y1, x2 - x1, y2 - y1);
            let face_path = format!("{}/face_{}_conf_{:.3}.jpg", output_dir, i + 1, confidence);
            cropped.save(&face_path)?;

            println!("Сохранено лицо: {}", face_path);
        }

        Ok(())
    }

    pub fn crop_face(&self, image: &DynamicImage, bbox: &BoundingBox) -> Result<DynamicImage> {
        let padding = 0.0;
        let x1 = (bbox.x1 - padding).max(0.0) as u32;
        let y1 = (bbox.y1 - padding).max(0.0) as u32;
        let x2 = (bbox.x2 + padding).min(image.width() as f32) as u32;
        let y2 = (bbox.y2 + padding).min(image.height() as f32) as u32;

        let cropped = image.crop_imm(x1, y1, x2 - x1, y2 - y1);
        Ok(cropped)
    }

    pub fn preprocess_image_for_recognition(&self, image: &DynamicImage) -> Result<Array4<f32>> {
        let img = image.resize_exact(160, 160, image::imageops::FilterType::Lanczos3);
        let img_rgb = img.to_rgb8();

        let mut input = Array4::<f32>::zeros((1, 3, 160, 160));

        for (y, row) in img_rgb.rows().enumerate() {
            for (x, pixel) in row.enumerate() {
                let [r, g, b] = pixel.0;
                input[[0, 0, y, x]] = (r as f32 - 127.5) / 128.0;
                input[[0, 1, y, x]] = (g as f32 - 127.5) / 128.0;
                input[[0, 2, y, x]] = (b as f32 - 127.5) / 128.0;
            }
        }

        Ok(input)
    }

    pub fn extract_face_embedding(&self, input_tensor: Array4<f32>) -> Result<Vec<f32>> {
        let input = inputs!["input.1"=>input_tensor]?;

        let outputs = self.recognition_session.run(input)?;

        let embedding_array = outputs["1197"]
            .try_extract_tensor::<f32>()?
            .t()
            .into_owned();

        let embedding: Vec<f32> = embedding_array.into_iter().collect();

        let norm = (embedding.iter().map(|x| x * x).sum::<f32>()).sqrt();
        let normalized_embedding: Vec<f32> = if norm > 0.0 {
            embedding.iter().map(|x| x / norm).collect()
        } else {
            embedding
        };

        println!("Размер эмбеддинга: {}", normalized_embedding.len());

        Ok(normalized_embedding)
    }

    fn cosine_similarity(&self, vec1: &[f32], vec2: &[f32]) -> f32 {
        if vec1.len() != vec2.len() {
            return 0.0;
        }

        let dot_product: f32 = vec1.iter().zip(vec2.iter()).map(|(a, b)| a * b).sum();
        let norm1: f32 = vec1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm2: f32 = vec2.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm1 == 0.0 || norm2 == 0.0 {
            return 0.0;
        }

        dot_product / (norm1 * norm2)
    }
}

#[derive(Debug, Clone)]
pub struct DatabasePerson {
    pub id: String,
    pub name: String,
    pub photo_path: String,
    pub face: Vec<f32>,
}

pub struct FaceDatabase {
    pub people: Vec<DatabasePerson>,
}

impl FaceDatabase {
    pub fn new() -> Self {
        Self { people: Vec::new() }
    }

    pub async fn add_person(
        &mut self,
        detections: &FaceRecognitionSystem,
        id: String,
        name: String,
        photo_path: String,
    ) -> Result<()> {
        let image = image::open(&photo_path)?;

        let faces = detections.detect_faces(&image).await?;

        if faces.is_empty() {
            return Err(anyhow!("На фотографии {} не найдено лиц", photo_path));
        }

        let crop_face = detections.crop_face(&image, &faces[0].0)?;

        let input_tensor = detections.preprocess_image_for_recognition(&crop_face)?;

        let emb = detections.extract_face_embedding(input_tensor)?;

        let person = DatabasePerson {
            id: id.clone(),
            name: name.clone(),
            photo_path,
            face: emb,
        };

        self.people.push(person);
        println!("Добавлен человек: {} (ID: {})", name, id);

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let face_system =
        FaceRecognitionSystem::new("models/yolov11s-face.onnx", "models/face-recognition.onnx")
            .await?;

    let face = r"assets/face_database/card_1/img_2.png";
    let test = r"assets/face_database/card_1/img_2.png";

    let mut database = FaceDatabase::new();

    match database
        .add_person(
            &face_system,
            "person1".to_string(),
            "Иван Иванов".to_string(),
            face.to_string(),
        )
        .await
    {
        Ok(_) => println!("✓ Успешно добавлен Иван Иванов"),
        Err(e) => println!("✗ Ошибка при добавлении Ивана Иванова: {}", e),
    }

    let test_image = image::open(test)?;

    let detections = face_system.detect_faces(&test_image).await?;

    for detection in &detections {
        let crop = face_system.crop_face(&test_image, &detection.0)?;
        let prepare = face_system.preprocess_image_for_recognition(&crop)?;
        let emb = face_system.extract_face_embedding(prepare)?;
        let sim = face_system.cosine_similarity(&emb, &database.people[0].face);

        println!("{:#?}", sim);
    }

    if !detections.is_empty() {
        face_system.draw_bounding_boxes(
            &test_image,
            &detections,
            "output/cristiano_with_boxes.jpg",
        )?;

        face_system.save_cropped_faces(&test_image, &detections, "output/faces")?;
    } else {
        println!("Лица не найдены на изображении");
    }

    Ok(())
}