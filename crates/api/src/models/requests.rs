use serde::{Deserialize, Serialize};
use storage::models::processing_job::ProcessingJob;
use storage::models::video_job::VideoJob;

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Serialize)]
pub struct ProcessingJobResponse {
    pub processing_job: ProcessingJob,
    pub video_jobs: Vec<VideoJob>,
}
