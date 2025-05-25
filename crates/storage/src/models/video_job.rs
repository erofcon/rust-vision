use crate::models::processing_job::ProcessingStatus;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, sqlx::FromRow, Debug)]
pub struct VideoJob {
    pub id: Uuid,
    pub processing_jobs_id: Uuid,
    pub camera_presets_id: Uuid,
    pub video_name: String,
    pub file_path: String,
    pub status: ProcessingStatus,
    pub start_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VideoJobCreate {
    pub processing_jobs_id: Uuid,
    pub camera_presets_id: Uuid,
    pub video_name: String,
    pub file_path: String,
    pub start_time: DateTime<Utc>,
}
