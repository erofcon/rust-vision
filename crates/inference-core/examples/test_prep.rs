use anyhow::Result;
use ndarray::Array4;
use opencv::core::{AlgorithmHint, MatTraitConst, MatTraitConstManual, Size, Size2i};
use opencv::{core::Mat, imgproc};
use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider};
use ort::session::Session;

// Ваш текущий вариант
fn preprocess_v1_current(image: &Mat) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
    let mut rgb_image = Mat::default();
    if image.channels() == 3 {
        imgproc::cvt_color(image, &mut rgb_image, imgproc::COLOR_BGR2RGB, 0, AlgorithmHint::ALGO_HINT_DEFAULT)?;
    } else {
        rgb_image = image.clone();
    }

    // Шаг 2: ВАЖНО! Изменяем размер на 160x160
    let mut resized_image = Mat::default();
    imgproc::resize(
        &rgb_image,
        &mut resized_image,
        Size2i::new(160, 160), // Правильный размер для вашей модели
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    )?;

    println!("Resized image: {}x{}", resized_image.cols(), resized_image.rows());

    // Шаг 3: Конвертируем в тензор
    let width = 160;
    let height = 160;
    let raw_data: Vec<u8> = resized_image.data_bytes()?.to_vec();

    let mut tensor = Array4::<f32>::zeros((1, 3, height, width));

    for y in 0..height {
        for x in 0..width {
            let pixel_pos = y * width * 3 + x * 3;
            if pixel_pos + 2 < raw_data.len() {
                // Нормализация [-1, 1]
                tensor[[0, 0, y, x]] = (raw_data[pixel_pos] as f32 - 127.5) / 128.0;     // R
                tensor[[0, 1, y, x]] = (raw_data[pixel_pos + 1] as f32 - 127.5) / 128.0; // G
                tensor[[0, 2, y, x]] = (raw_data[pixel_pos + 2] as f32 - 127.5) / 128.0; // B
            }
        }
    }

    println!("Tensor shape: {:?}", tensor.shape());
    println!("Tensor stats: min={:.3}, max={:.3}, mean={:.3}",
             tensor.iter().cloned().fold(f32::INFINITY, f32::min),
             tensor.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
             tensor.iter().sum::<f32>() / tensor.len() as f32);

    Ok(tensor)
}

// Вариант с нормализацией [0, 1]
fn preprocess_v2_zero_one(image: &Mat) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
    let mut rgb_image = Mat::default();
    if image.channels() == 3 {
        imgproc::cvt_color(
            image,
            &mut rgb_image,
            imgproc::COLOR_BGR2RGB,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;
    } else {
        rgb_image = image.clone();
    }

    let width = rgb_image.cols() as usize;
    let height = rgb_image.rows() as usize;
    let raw_data: Vec<u8> = rgb_image.data_bytes()?.to_vec();

    let mut tensor = Array4::<f32>::zeros((1, 3, height, width));

    for y in 0..height {
        for x in 0..width {
            let pixel_pos = y * width * 3 + x * 3;
            if pixel_pos + 2 < raw_data.len() {
                tensor[[0, 0, y, x]] = raw_data[pixel_pos] as f32 / 255.0;
                tensor[[0, 1, y, x]] = raw_data[pixel_pos + 1] as f32 / 255.0;
                tensor[[0, 2, y, x]] = raw_data[pixel_pos + 2] as f32 / 255.0;
            }
        }
    }

    Ok(tensor)
}

// Вариант без конвертации цветов (BGR как есть)
fn preprocess_v3_bgr(image: &Mat) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
    let width = image.cols() as usize;
    let height = image.rows() as usize;
    let raw_data: Vec<u8> = image.data_bytes()?.to_vec();

    let mut tensor = Array4::<f32>::zeros((1, 3, height, width));

    for y in 0..height {
        for x in 0..width {
            let pixel_pos = y * width * 3 + x * 3;
            if pixel_pos + 2 < raw_data.len() {
                // BGR порядок
                tensor[[0, 0, y, x]] = (raw_data[pixel_pos] as f32 - 127.5) / 128.0; // B
                tensor[[0, 1, y, x]] = (raw_data[pixel_pos + 1] as f32 - 127.5) / 128.0; // G
                tensor[[0, 2, y, x]] = (raw_data[pixel_pos + 2] as f32 - 127.5) / 128.0; // R
            }
        }
    }

    Ok(tensor)
}

// ImageNet стандартная нормализация
fn preprocess_v4_imagenet(image: &Mat) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
    let mut rgb_image = Mat::default();
    if image.channels() == 3 {
        imgproc::cvt_color(
            image,
            &mut rgb_image,
            imgproc::COLOR_BGR2RGB,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )?;
    } else {
        rgb_image = image.clone();
    }

    let width = rgb_image.cols() as usize;
    let height = rgb_image.rows() as usize;
    let raw_data: Vec<u8> = rgb_image.data_bytes()?.to_vec();

    // ImageNet mean and std
    let mean = [0.485, 0.456, 0.406]; // RGB
    let std = [0.229, 0.224, 0.225]; // RGB

    let mut tensor = Array4::<f32>::zeros((1, 3, height, width));

    for y in 0..height {
        for x in 0..width {
            let pixel_pos = y * width * 3 + x * 3;
            if pixel_pos + 2 < raw_data.len() {
                let r = (raw_data[pixel_pos] as f32 / 255.0 - mean[0]) / std[0];
                let g = (raw_data[pixel_pos + 1] as f32 / 255.0 - mean[1]) / std[1];
                let b = (raw_data[pixel_pos + 2] as f32 / 255.0 - mean[2]) / std[2];

                tensor[[0, 0, y, x]] = r;
                tensor[[0, 1, y, x]] = g;
                tensor[[0, 2, y, x]] = b;
            }
        }
    }

    Ok(tensor)
}

// Функция для тестирования всех вариантов
fn test_all_preprocessing(session: &Session, image: &Mat, name: &str) -> Result<()> {
    println!("\n=== Testing preprocessing variants for: {} ===", name);

    let variants = [
        (
            "Current [-1,1] RGB",
            preprocess_v1_current as fn(&Mat) -> Result<Array4<f32>, Box<dyn std::error::Error>>,
        ),
        ("Zero-One [0,1] RGB", preprocess_v2_zero_one),
        ("Current [-1,1] BGR", preprocess_v3_bgr),
        ("ImageNet norm RGB", preprocess_v4_imagenet),
    ];

    let mut embeddings = Vec::new();

    for (variant_name, preprocess_fn) in variants.iter() {
        match preprocess_fn(image) {
            Ok(tensor) => {
                println!("\n--- {} ---", variant_name);

                // Статистика входного тензора
                let min_val = tensor.iter().cloned().fold(f32::INFINITY, f32::min);
                let max_val = tensor.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let mean_val = tensor.iter().sum::<f32>() / tensor.len() as f32;

                println!(
                    "Input tensor stats: min={:.3}, max={:.3}, mean={:.3}",
                    min_val, max_val, mean_val
                );

                match extract_face_embedding_debug(session, tensor) {
                    Ok(embedding) => {
                        embeddings.push((variant_name.to_string(), embedding));
                        println!("✓ Successfully extracted embedding");
                    }
                    Err(e) => {
                        println!("✗ Failed to extract embedding: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("✗ Preprocessing failed for {}: {}", variant_name, e);
            }
        }
    }

    // Сравниваем эмбеддинги между собой
    println!("\n=== Cross-variant similarity comparison ===");
    for i in 0..embeddings.len() {
        for j in (i + 1)..embeddings.len() {
            let (name1, emb1) = &embeddings[i];
            let (name2, emb2) = &embeddings[j];

            if let Ok(similarity) = cosine_similarity(emb1, emb2) {
                println!("{} vs {}: {:.6}", name1, name2, similarity);
            }
        }
    }

    Ok(())
}

// Функция для тестирования на одинаковых изображениях
fn test_same_image_consistency(session: &Session, image: &Mat) -> Result<()> {
    println!("\n=== Same Image Consistency Test ===");

    // Извлекаем эмбеддинг дважды из одного изображения
    let tensor1 = preprocess_v1_current(image).unwrap();
    let tensor2 = preprocess_v1_current(image).unwrap();

    let emb1 = extract_face_embedding_debug(session, tensor1)?;
    let emb2 = extract_face_embedding_debug(session, tensor2)?;

    let similarity = cosine_similarity(&emb1, &emb2)?;
    println!("Same image similarity: {:.10}", similarity);

    if similarity < 0.99999 {
        println!("WARNING: Same image produces different embeddings!");
    } else {
        println!("✓ Consistency check passed");
    }

    Ok(())
}

// Вспомогательные функции из предыдущего кода
fn extract_face_embedding_debug(
    session: &Session,
    input_tensor: Array4<f32>,
) -> anyhow::Result<Vec<f32>> {
    let input = ort::inputs!["input.1" => input_tensor]?;
    let outputs = session.run(input)?;
    let output_tensor = outputs["1197"].try_extract_tensor::<f32>()?;
    let embedding: Vec<f32> = output_tensor.iter().cloned().collect();

    println!(
        "Embedding stats: len={}, min={:.6}, max={:.6}, mean={:.6}",
        embedding.len(),
        embedding.iter().cloned().fold(f32::INFINITY, f32::min),
        embedding.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
        embedding.iter().sum::<f32>() / embedding.len() as f32
    );

    Ok(embedding)
}

pub fn load_model(detection_model_path: &str) -> Result<Session> {
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


fn preprocess_image_correctly(image: &Mat, target_size: i32) -> Result<Array4<f32>, Box<dyn std::error::Error>> {
    // Ресайз изображения
    let mut resized = Mat::default();
    imgproc::resize(
        image,
        &mut resized,
        Size::new(target_size, target_size),
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    )?;

    println!("Resized image: {}x{}", resized.cols(), resized.rows());

    // Конвертация в RGB
    let mut rgb_image = Mat::default();
    if resized.channels() == 3 {
        imgproc::cvt_color(&resized, &mut rgb_image, imgproc::COLOR_BGR2RGB, 0, AlgorithmHint::ALGO_HINT_DEFAULT)?;
    } else {
        rgb_image = resized.clone();
    }

    let width = rgb_image.cols() as usize;
    let height = rgb_image.rows() as usize;
    let raw_data: Vec<u8> = rgb_image.data_bytes()?.to_vec();

    let channel_size = width * height;
    let total_size = 3 * channel_size;
    let mut flat_data = vec![0.0f32; total_size];

    let (r_channel, rest) = flat_data.split_at_mut(channel_size);
    let (g_channel, b_channel) = rest.split_at_mut(channel_size);

    for y in 0..height {
        for x in 0..width {
            let pixel_pos = y * width * 3 + x * 3;
            if pixel_pos + 2 < raw_data.len() {
                let idx = y * width + x;
                r_channel[idx] = (raw_data[pixel_pos] as f32 - 127.5) / 128.0;
                g_channel[idx] = (raw_data[pixel_pos + 1] as f32 - 127.5) / 128.0;
                b_channel[idx] = (raw_data[pixel_pos + 2] as f32 - 127.5) / 128.0;
            }
        }
    }

    let tensor = Array4::from_shape_vec((1, 3, height, width), flat_data)?;

    // Статистика тензора
    let min_val = tensor.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_val = tensor.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let mean_val = tensor.iter().sum::<f32>() / tensor.len() as f32;
    println!("Tensor shape: {:?}", tensor.shape());
    println!("Tensor stats: min={:.3}, max={:.3}, mean={:.3}", min_val, max_val, mean_val);

    Ok(tensor)
}

// Улучшенная функция извлечения эмбеддинга
fn extract_face_embedding_improved(
    session: &Session,
    input_tensor: Array4<f32>,
    debug: bool,
) -> anyhow::Result<Vec<f32>> {
    let input = ort::inputs!["input.1" => input_tensor]?;
    let outputs = session.run(input)?;
    let output_tensor = outputs["1197"].try_extract_tensor::<f32>()?;
    let embedding: Vec<f32> = output_tensor.iter().cloned().collect();

    if debug {
        println!("Embedding stats: len={}, min={:.6}, max={:.6}, mean={:.6}",
                 embedding.len(),
                 embedding.iter().cloned().fold(f32::INFINITY, f32::min),
                 embedding.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
                 embedding.iter().sum::<f32>() / embedding.len() as f32);
    }

    // Проверки валидности
    if embedding.iter().any(|&x| !x.is_finite()) {
        return Err(anyhow::anyhow!("Embedding contains invalid values"));
    }

    Ok(embedding)
}

// Функция для определения оптимального порога
fn find_optimal_threshold(
    session: &Session,
    same_person_pairs: &[(Mat, Mat)],  // Пары изображений одного человека
    different_person_pairs: &[(Mat, Mat)], // Пары изображений разных людей
) -> Result<f32> {
    println!("=== Finding Optimal Threshold ===");

    let mut same_person_similarities = Vec::new();
    let mut different_person_similarities = Vec::new();

    // Тестируем пары одного человека
    println!("Testing same person pairs...");
    for (i, (img1, img2)) in same_person_pairs.iter().enumerate() {
        let tensor1 = preprocess_image_correctly(img1, 160).unwrap();
        let tensor2 = preprocess_image_correctly(img2, 160).unwrap();

        let emb1 = extract_face_embedding_improved(session, tensor1, false)?;
        let emb2 = extract_face_embedding_improved(session, tensor2, false)?;

        let similarity = cosine_similarity(&emb1, &emb2)?;
        same_person_similarities.push(similarity);
        println!("Same person pair {}: {:.4}", i + 1, similarity);
    }

    // Тестируем пары разных людей
    println!("Testing different person pairs...");
    for (i, (img1, img2)) in different_person_pairs.iter().enumerate() {
        let tensor1 = preprocess_image_correctly(img1, 160).unwrap();
        let tensor2 = preprocess_image_correctly(img2, 160).unwrap();

        let emb1 = extract_face_embedding_improved(session, tensor1, false)?;
        let emb2 = extract_face_embedding_improved(session, tensor2, false)?;

        let similarity = cosine_similarity(&emb1, &emb2)?;
        different_person_similarities.push(similarity);
        println!("Different person pair {}: {:.4}", i + 1, similarity);
    }

    // Анализ результатов
    if same_person_similarities.is_empty() || different_person_similarities.is_empty() {
        return Err(anyhow::anyhow!("Need both same and different person pairs"));
    }

    let min_same = same_person_similarities.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_same = same_person_similarities.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let avg_same = same_person_similarities.iter().sum::<f32>() / same_person_similarities.len() as f32;

    let min_diff = different_person_similarities.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_diff = different_person_similarities.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let avg_diff = different_person_similarities.iter().sum::<f32>() / different_person_similarities.len() as f32;

    println!("\n=== Results Analysis ===");
    println!("Same person similarities: min={:.4}, max={:.4}, avg={:.4}", min_same, max_same, avg_same);
    println!("Different person similarities: min={:.4}, max={:.4}, avg={:.4}", min_diff, max_diff, avg_diff);

    // Находим оптимальный порог
    let optimal_threshold = if max_diff < min_same {
        // Идеальный случай - есть четкий разрыв
        (max_diff + min_same) / 2.0
    } else {
        // Находим компромисс
        let mut best_threshold = 0.5;
        let mut best_accuracy = 0.0;

        for threshold in (0..100).map(|x| x as f32 / 100.0) {
            let tp = same_person_similarities.iter().filter(|&&s| s >= threshold).count();
            let tn = different_person_similarities.iter().filter(|&&s| s < threshold).count();
            let accuracy = (tp + tn) as f32 / (same_person_similarities.len() + different_person_similarities.len()) as f32;

            if accuracy > best_accuracy {
                best_accuracy = accuracy;
                best_threshold = threshold;
            }
        }

        println!("Best threshold: {:.3} with accuracy: {:.3}", best_threshold, best_accuracy);
        best_threshold
    };

    println!("Recommended threshold: {:.3}", optimal_threshold);
    Ok(optimal_threshold)
}

// Функция для тестирования модели
fn comprehensive_test(
    session: &Session,
    test_images: &[(&str, Mat)], // (имя_человека, изображение)
) -> Result<()> {
    println!("=== Comprehensive Face Recognition Test ===");

    let mut embeddings = Vec::new();

    // Извлекаем эмбеддинги для всех изображений
    for (name, image) in test_images.iter() {
        println!("Processing: {}", name);
        let tensor = preprocess_image_correctly(image, 160).unwrap();
        let embedding = extract_face_embedding_improved(session, tensor, true)?;
        embeddings.push((name.to_string(), embedding));
    }

    // Создаем матрицу сходства
    println!("\n=== Similarity Matrix ===");
    print!("{:>15}", "");
    for (name, _) in embeddings.iter() {
        print!("{:>10}", name);
    }
    println!();

    for (i, (name1, emb1)) in embeddings.iter().enumerate() {
        print!("{:>15}", name1);
        for (j, (_, emb2)) in embeddings.iter().enumerate() {
            let similarity = cosine_similarity(emb1, emb2).unwrap_or(0.0);
            if i == j {
                print!("{:>10}", "1.000"); // Сам с собой
            } else {
                print!("{:>10.3}", similarity);
            }
        }
        println!();
    }

    // Анализ результатов
    println!("\n=== Analysis ===");
    for (i, (name1, emb1)) in embeddings.iter().enumerate() {
        for (j, (name2, emb2)) in embeddings.iter().enumerate() {
            if i < j {
                let similarity = cosine_similarity(emb1, emb2)?;
                let same_person = name1.split('_').next() == name2.split('_').next();
                let status = if same_person && similarity > 0.4 {
                    "✓ Correct (Same person)"
                } else if !same_person && similarity <= 0.4 {
                    "✓ Correct (Different people)"
                } else if same_person && similarity <= 0.4 {
                    "✗ False Negative (Should be same)"
                } else {
                    "✗ False Positive (Should be different)"
                };

                println!("{} vs {}: {:.4} - {}", name1, name2, similarity, status);
            }
        }
    }

    Ok(())
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


fn main() -> Result<()> {
    let model_path = "models/face-recognition.onnx";

    let session = load_model(model_path)?;

    let test_images = vec![
        ("person1_photo1", opencv::imgcodecs::imread("assets/face_database/people_1/img.png", opencv::imgcodecs::IMREAD_COLOR)?),
        ("person1_photo2", opencv::imgcodecs::imread("assets/face_database/people_1/img_1.png", opencv::imgcodecs::IMREAD_COLOR)?),
        ("person2_photo1", opencv::imgcodecs::imread("assets/face_database/people_1/img_2.png", opencv::imgcodecs::IMREAD_COLOR)?),
        ("person2_photo2", opencv::imgcodecs::imread("assets/face_database/people_1/img_3.png", opencv::imgcodecs::IMREAD_COLOR)?),
    ];

    // Комплексный тест
    comprehensive_test(&session, &test_images)?;

    // Загрузите несколько тестовых изображений
    // let test_image1 = opencv::imgcodecs::imread(
    //     "assets/face_database/people_1/img.png",
    //     opencv::imgcodecs::IMREAD_COLOR,
    // )?;
    // let test_image2 = opencv::imgcodecs::imread(
    //     "assets/face_database/people_1/img_1.png",
    //     opencv::imgcodecs::IMREAD_COLOR,
    // )?;
    //
    // // Тест консистентности на одном изображении
    // test_same_image_consistency(&session, &test_image1)?;
    //
    // // Тест разных вариантов предобработки
    // test_all_preprocessing(&session, &test_image1, "Person1")?;
    // test_all_preprocessing(&session, &test_image2, "Person2")?;

    Ok(())
}
