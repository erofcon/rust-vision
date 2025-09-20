use crate::detectors::{Detectors, Inference, Output, Prepare};
use crate::globals::{FACE_DETECTION_MODEL, FACE_RECOGNITION_MODEL};
use anyhow::{Result, anyhow};
use image::GenericImageView;
use project_config::global::PROJECT_CONFIG;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Write;
use opencv::imgcodecs;
use opencv::prelude::MatTraitConst;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabasePerson {
    pub name: String,
    pub emb: Vec<f32>,
    pub individual_embs: Vec<Vec<f32>>,
    pub emb_count: usize,
}

impl DatabasePerson {
    pub fn new(name: String) -> Self {
        Self {
            name,
            emb: Vec::new(),
            individual_embs: Vec::new(),
            emb_count: 0,
        }
    }

    pub fn add_embedding(&mut self, new_emb: Vec<f32>) {
        self.individual_embs.push(new_emb.clone());

        if self.emb.is_empty() {
            self.emb = new_emb;
            self.emb_count = 1;
        } else {
            for (i, &new_val) in new_emb.iter().enumerate() {
                self.emb[i] =
                    (self.emb[i] * self.emb_count as f32 + new_val) / (self.emb_count + 1) as f32;
            }
            self.emb_count += 1;
        }
    }

    pub fn recalculate_average(&mut self) {
        if self.individual_embs.is_empty() {
            return;
        }

        let emb_size = self.individual_embs[0].len();
        let mut avg_emb = vec![0.0; emb_size];

        for emb in &self.individual_embs {
            for (i, &val) in emb.iter().enumerate() {
                avg_emb[i] += val;
            }
        }

        for val in &mut avg_emb {
            *val /= self.individual_embs.len() as f32;
        }

        self.emb = avg_emb;
        self.emb_count = self.individual_embs.len();
    }

    pub fn get_best_embedding(&self) -> &Vec<f32> {
        &self.emb
    }

    pub fn get_all_embeddings(&self) -> &Vec<Vec<f32>> {
        &self.individual_embs
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceDatabase {
    pub people: Vec<DatabasePerson>,
}

impl FaceDatabase {
    pub fn new() -> Self {
        Self { people: Vec::new() }
    }

    pub fn load_from_folder(&mut self) -> Result<()> {
        let base_dir = &PROJECT_CONFIG.face_database.path;
        let face_models = FACE_DETECTION_MODEL.get_session();
        let face_recognition_model = FACE_RECOGNITION_MODEL.get_session();
        let target_width = PROJECT_CONFIG.face_detection_model.input_width.clone() as usize;
        let target_height = PROJECT_CONFIG.face_detection_model.input_height.clone() as usize;

        for entry in fs::read_dir(base_dir)? {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }

            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let dir = path.clone();
            let mut person = DatabasePerson::new(name);

            println!("Processing person: {}", person.name);

            for img_e in fs::read_dir(&dir)? {
                let p = img_e?.path();
                if !p.is_file() {
                    continue;
                }

                if let Some(ext) = p.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if !["jpg", "jpeg", "png", "bmp", "gif", "tiff"].contains(&ext_str.as_str()) {
                        continue;
                    }
                }

                println!("Processing image: {:?}", p.to_str().unwrap());

                let image = match imgcodecs::imread(p.to_str().unwrap(), imgcodecs::IMREAD_COLOR) {
                    Ok(img) => img,
                    Err(e) => {
                        eprintln!("  Error opening image {:?}: {}", p, e);
                        continue;
                    }
                };

                let width = image.cols();
                let height = image.rows();

                let (resized_img, scale, pad_x, pad_y) =
                    Detectors::resize_mat_with_padding(&image)?;

                let prepared =
                    Detectors::prepare_mat_img(&resized_img, target_width, target_height)
                        .map_err(|e| anyhow!("prepare_dynamic_image failed: {:?}", e))?;

                let results = Detectors::inference(face_models, prepared)?;

                let detections =
                    Detectors::process_detections(&results, width, height, scale, pad_x, pad_y)?;

                if detections.is_empty() {
                    println!("No faces detected in {:?}", p);
                    continue;
                }

                let best_face = detections.iter().max_by(|a, b| {
                    a.confidence
                        .partial_cmp(&b.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                if let Some(detection) = best_face {
                    let crop_face = match Detectors::crop_detection(&image, &detection) {
                        Ok(crop) => crop,
                        Err(e) => {
                            eprintln!("  Error cropping face from {:?}: {}", p, e);
                            continue;
                        }
                    };

                    let aligned_face = Detectors::align_face(&crop_face, detection)?;

                    let rec_model_w = PROJECT_CONFIG.face_recognition_model.input_width as i32;
                    let rec_model_h = PROJECT_CONFIG.face_recognition_model.input_height as i32;

                    let recognition_face =
                        Detectors::resize_mat(&aligned_face, rec_model_w, rec_model_h)?;

                    let prepare_rec =
                        match Detectors::preprocess_image_for_recognition_mat(&recognition_face) {
                            Ok(prep) => prep,
                            Err(e) => {
                                eprintln!("  Error preprocessing for recognition {:?}: {}", p, e);
                                continue;
                            }
                        };

                    let emb = match Detectors::extract_face_embedding(
                        face_recognition_model,
                        prepare_rec,
                    ) {
                        Ok(embedding) => embedding,
                        Err(e) => {
                            eprintln!("  Error extracting embedding from {:?}: {}", p, e);
                            continue;
                        }
                    };

                    person.add_embedding(emb);
                    println!("  Successfully processed face from {:?}", p);
                } else {
                    println!("  No valid face found in {:?}", p);
                }
            }

            if person.emb_count > 0 {
                println!(
                    "Added person '{}' with {} embeddings",
                    person.name, person.emb_count
                );
                self.people.push(person);
            } else {
                println!(
                    "Skipped person '{}' - no valid face embeddings found",
                    person.name
                );
            }
        }

        Ok(())
    }

    pub fn save_to_file(&self) -> Result<()> {
        let path = &PROJECT_CONFIG.face_database.db_file;

        let j = serde_json::to_string_pretty(&self)?;
        let mut f = File::create(path)?;
        f.write_all(j.as_bytes())?;

        println!("Face database saved to: {:?}", path);
        println!("Total people in database: {}", self.people.len());

        Ok(())
    }

    pub fn load_from_file() -> Result<Self> {
        let path = &PROJECT_CONFIG.face_database.db_file;

        let s = fs::read_to_string(path)?;
        let mut db: FaceDatabase = serde_json::from_str(&s)?;

        for person in &mut db.people {
            if !person.individual_embs.is_empty() {
                person.recalculate_average();
            }
        }

        println!("Face database loaded from: {:?}", path);
        println!("Total people in database: {}", db.people.len());

        Ok(db)
    }
}
