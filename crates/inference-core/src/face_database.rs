use crate::detectors::{Detectors, Inference, Output, Prepare};
use crate::globals::{FACE_DETECTION_MODEL, FACE_RECOGNITION_MODEL};
use anyhow::Result;
use common::config::ProjectConfig;
use image::GenericImageView;
use project_config::global::PROJECT_CONFIG;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::Write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabasePerson {
    pub name: String,
    pub emb: Vec<Vec<f32>>,
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

        for entry in fs::read_dir(base_dir)? {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let dir = path.clone();
            let mut embeddings = Vec::new();

            for img_e in fs::read_dir(&dir)? {
                let p = img_e?.path();
                if !p.is_file() {
                    continue;
                }

                let img = image::open(&p)?;

                let prepare = Detectors::prepare_dynamic_image_for_yolo11s(&img).unwrap();

                let inference = Detectors::yolo11_inference(face_models, prepare)?;

                let results = Detectors::process_yolo11s_output(
                    inference,
                    img.width() as f32,
                    img.height() as f32,
                    None,
                    None,
                )?;

                if results.is_empty() {
                    continue;
                }

                let crop = Detectors::crop_face(&img, &results[0].0)?;

                let prepare_rec = Detectors::preprocess_image_for_recognition(&crop).unwrap();
                let emb = Detectors::extract_face_embedding(face_recognition_model, prepare_rec)?;

                embeddings.push(emb);
            }

            self.people.push(DatabasePerson {
                name,
                emb: embeddings,
            });
        }

        Ok(())
    }

    pub fn save_to_file(&self) -> Result<()> {
        let path = &PROJECT_CONFIG.face_database.db_file;

        let j = serde_json::to_string_pretty(&self)?;
        let mut f = File::create(path)?;
        f.write_all(j.as_bytes())?;
        Ok(())
    }

    pub fn load_from_file() -> Result<Self> {
        let path = &PROJECT_CONFIG.face_database.db_file;

        let s = fs::read_to_string(path)?;
        let db: FaceDatabase = serde_json::from_str(&s)?;
        Ok(db)
    }
}
