use anyhow::{Context, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub db_host: String,
    pub db_port: u16,
    pub db_user: String,
    pub db_password: String,
    pub db_name: String,
    pub upload_dir: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT")
            .unwrap_or_else(|_| "9876".to_string())
            .parse::<u16>()
            .context("Failed to parse PORT")?;

        let db_host = env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db_port = env::var("DB_PORT")
            .unwrap_or_else(|_| "5432".to_string())
            .parse::<u16>()
            .context("Failed to parse DB_PORT")?;
        let db_user = env::var("DB_USER").unwrap_or_else(|_| "myuser".to_string());
        let db_password = env::var("DB_PASSWORD").unwrap_or_else(|_| "mypassword".to_string());
        let db_name = env::var("DB_NAME").unwrap_or_else(|_| "myapp".to_string());

        let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".to_string());

        std::fs::create_dir_all(&upload_dir).context("Failed to create upload directory")?;

        Ok(Self {
            host,
            port,
            db_host,
            db_port,
            db_user,
            db_password,
            db_name,
            upload_dir,
        })
    }
}
