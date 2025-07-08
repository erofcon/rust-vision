pub mod global;

use anyhow::Result;
use config::{Config, ConfigError, File};
use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Clone)]
pub struct Api {
    pub host: String,
    pub port: u16,
    pub upload_dir: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Database {
    pub url: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub migrations_path: String,
}

impl Database {
    pub fn connection_string(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, self.password, self.url, self.port, self.database
        )
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct MQ {
    pub url: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

impl MQ {
    pub fn connection_to_string(&self) -> String {
        format!(
            "amqp://{}:{}@{}:{}",
            self.user, self.password, self.url, self.port
        )
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ObjectDetectionModel {
    pub path: String,
    pub input_width: i32,
    pub input_height: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FaceDetectionModel {
    pub path: String,
    pub input_width: i32,
    pub input_height: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FaceRecognitionModel {
    pub path: String,
    pub input_width: i32,
    pub input_height: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FaceDatabase {
    pub path: String,
    pub db_file: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Worker {
    pub worker_count: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProjectConfig {
    pub api: Api,
    pub database: Database,
    pub mq: MQ,
    pub object_detection_model: ObjectDetectionModel,
    pub face_detection_model: FaceDetectionModel,
    pub face_recognition_model: FaceRecognitionModel,
    pub face_database: FaceDatabase,
    pub worker: Worker,
}

impl ProjectConfig {

    pub fn load() -> Result<ProjectConfig, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "debug".into());
        let config_dir = env::var("CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let config_path = Path::new(&config_dir);

        if !config_path.exists() {
            return Err(ConfigError::NotFound(format!(
                "Common configuration directory not found: {}",
                config_path.display()
            )));
        };

        let file = config_path.join(format!("{}.toml", run_mode));
        let config = Config::builder().add_source(File::from(file)).build()?;

        let mut project_config: ProjectConfig = config.try_deserialize()?;

        let project_root = Self::find_project_root()?;
        project_config.resolve_paths(&project_root);

        Ok(project_config)
    }

    // pub fn load() -> Result<ProjectConfig, ConfigError> {
    //     let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "debug".into());
    //     let config_dir = env::var("CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
    //     let config_path = Path::new(&config_dir);
    //
    //     if !config_path.exists() {
    //         return Err(ConfigError::NotFound(format!(
    //             "Common configuration directory not found: {}",
    //             config_path.display()
    //         )));
    //     };
    //
    //     let file = config_path.join(format!("{}.toml", run_mode));
    //     let config = Config::builder().add_source(File::from(file)).build()?;
    //
    //     config.try_deserialize()
    // }

    fn find_project_root() -> Result<PathBuf, ConfigError> {
        env::current_dir()
            .map_err(|e| ConfigError::Message(format!("Failed to get current directory: {}", e)))?
            .ancestors()
            .find(|p| p.join("Cargo.toml").exists())
            .map(|p| p.to_path_buf())
            .ok_or_else(|| ConfigError::Message("Cargo.toml not found".to_string()))
    }

    fn resolve_paths(&mut self, project_root: &Path) {
        let paths = [
            &mut self.api.upload_dir,
            &mut self.database.migrations_path,
            &mut self.object_detection_model.path,
            &mut self.face_detection_model.path,
            &mut self.face_recognition_model.path,
            &mut self.face_database.path,
            &mut self.face_database.db_file,
        ];

        for path in paths {
            if !Path::new(path).is_absolute() {
                *path = project_root.join(&*path).to_string_lossy().to_string();
            }
        }
    }

}
