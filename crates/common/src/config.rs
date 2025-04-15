use config::{Config, ConfigError, File};
use serde::Deserialize;
use std::env;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub migrations_path: String,
}

impl DatabaseConfig {
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
    pub fn connection_string(&self) -> String {
        format!(
            "amqp://{}:{}@{}:{}",
            self.user, self.password, self.url, self.port
        )
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommonConfig {
    pub database: DatabaseConfig,
    pub mq: MQ,
}

impl CommonConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "debug".into());
        let config_dir = env::var("CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let config_path = Path::new(&config_dir).join("common");

        if !config_path.exists() {
            return Err(ConfigError::NotFound(format!(
                "Common configuration directory not found: {}",
                config_path.display()
            )));
        };

        let file = config_path.join(format!("{}.toml", run_mode));

        let config = Config::builder().add_source(File::from(file)).build()?;

        config.try_deserialize()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub upload_dir: String,
}

impl ApiConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "debug".into());
        let config_dir = env::var("CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let config_path = Path::new(&config_dir).join("api");

        if !config_path.exists() {
            return Err(ConfigError::NotFound(format!(
                "Api configuration directory not found: {}",
                config_path.display()
            )));
        };

        let file = config_path.join(format!("{}.toml", run_mode));

        let config = Config::builder().add_source(File::from(file)).build()?;

        config.try_deserialize()
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelConfig {
    pub input_width: i32,
    pub input_height: i32,
}

impl ModelConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "debug".into());
        let config_dir = env::var("CONFIG_DIR").unwrap_or_else(|_| "config".to_string());
        let config_path = Path::new(&config_dir).join("model");

        if !config_path.exists() {
            return Err(ConfigError::NotFound(format!(
                "Model configuration directory not found: {}",
                config_path.display()
            )));
        };

        let file = config_path.join(format!("{}.toml", run_mode));

        let config = Config::builder().add_source(File::from(file)).build()?;

        config.try_deserialize()
    }
}
