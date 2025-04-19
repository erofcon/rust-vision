use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(type_name = "video_status")]
#[sqlx(rename_all = "lowercase")]
pub enum VideoStatus {
    Uploaded,
    Waiting,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct Video {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
    pub status: VideoStatus,
}
