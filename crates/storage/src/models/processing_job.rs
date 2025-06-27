use crate::models::video_job::VideoJob;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "processing_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ProcessingStatus {
    Uploaded,
    Waiting,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Serialize, Deserialize)]
pub struct CreateProcessingJob {
    pub organization_id: Uuid,
    pub day_map_id: Uuid,
    pub video_folder_path: String,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct ProcessingJob {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub day_map_id: Uuid,
    pub video_folder_path: String,
    pub status: ProcessingStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct ProcessingJobWithVideoJobResponse {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub day_map_id: Uuid,
    pub video_folder_path: String,
    pub status: ProcessingStatus,
    pub created_at: DateTime<Utc>,
    pub video_jobs: Vec<VideoJob>,
}
