use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabasePerson {
    pub id: String,
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

    pub async fn load_from_folder(&mut self, base_dir: &str) -> Result<()> {
        // let mut tasks = Vec::new();
        for entry in fs::read_dir(base_dir)? {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();

            return Ok(());
        }

        Ok(())
    }
}
