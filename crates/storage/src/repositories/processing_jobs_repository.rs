use crate::models::processing_job::{CreateProcessingJob, ProcessingJob, ProcessingStatus};
use crate::models::video_job::{VideoJob, VideoJobCreate};
use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

pub struct ProcessingJobsRepository {
    pool: Pool<Postgres>,
}

impl ProcessingJobsRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    // TODO: убрать во всех api отдельные UUID. Перемести в модель
    pub async fn create_processing_jobs(&self, job: &CreateProcessingJob) -> Result<ProcessingJob> {
        let result = sqlx::query_as(
            r#"
            INSERT INTO processing_jobs (id, organization_id, day_map_id, video_folder_path, status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
            .bind(&Uuid::new_v4())
            .bind(&job.organization_id)
            .bind(&job.day_map_id)
            .bind(&job.video_folder_path)
            .bind(&ProcessingStatus::Uploaded)
            .bind(&Utc::now())
            .fetch_one(&self.pool)
            .await.context("Error to create processing_jobs")
            ?;

        Ok(result)
    }

    pub async fn create_video_job(&self, video_job: &VideoJobCreate) -> Result<VideoJob> {
        let result = sqlx::query_as(
            r#"
            INSERT INTO video_jobs (id, processing_jobs_id, camera_presets_id, video_name, file_path, status, start_time, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
            .bind(&Uuid::new_v4())
            .bind(&video_job.processing_jobs_id)
            .bind(&video_job.camera_presets_id)
            .bind(&video_job.video_name)
            .bind(&video_job.file_path)
            .bind(ProcessingStatus::Uploaded)
            .bind(&video_job.start_time)
            .bind(&Utc::now())
            .fetch_one(&self.pool)
            .await?;

        Ok(result)
    }
}
