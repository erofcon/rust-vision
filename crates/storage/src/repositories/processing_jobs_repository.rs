use crate::models::processing_job::{
    CreateProcessingJob, ProcessingJob, ProcessingJobWithVideoJobResponse, ProcessingStatus,
};
use crate::models::video::{Video, VideoStatus};
use crate::models::video_job::{VideoJob, VideoJobCreate};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Pool, Postgres, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct ProcessingJobsRepository {
    pool: Pool<Postgres>,
}

impl ProcessingJobsRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    // TODO: убрать во всех api отдельные UUID. Перенести в модель
    // TODO: необходимо убрать status (не используется)
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
            .await.context("Error to create video_job")?;

        Ok(result)
    }

    pub async fn get_processing_job_with_video_job(
        &self,
        processing_id: &Uuid,
    ) -> Result<ProcessingJobWithVideoJobResponse> {
        let row = sqlx::query(
            r#"
        SELECT
            pj.id,
            pj.organization_id,
            pj.day_map_id,
            pj.video_folder_path,
            pj.status,
            pj.created_at,
            (
                SELECT COALESCE(
                    json_agg(
                        json_build_object(
                            'id', vj.id,
                            'processing_jobs_id', vj.processing_jobs_id,
                            'camera_presets_id', vj.camera_presets_id,
                            'video_name', vj.video_name,
                            'file_path', vj.file_path,
                            'status', vj.status::text,
                            'start_time', vj.start_time,
                            'created_at', vj.created_at
                        )
                    ),
                    '[]'::json
                )
                FROM video_jobs vj
                WHERE vj.processing_jobs_id = pj.id
            ) as video_jobs_json
        FROM processing_jobs pj
        WHERE pj.id = $1
        "#,
        )
        .bind(processing_id)
        .fetch_one(&self.pool)
        .await?;

        let id: Uuid = row.get("id");
        let organization_id: Uuid = row.get("organization_id");
        let day_map_id: Uuid = row.get("day_map_id");
        let video_folder_path: String = row.get("video_folder_path");
        let status: ProcessingStatus = row.get("status");
        let created_at: DateTime<Utc> = row.get("created_at");

        let video_jobs_json: Value = row.get("video_jobs_json");
        let video_jobs: Vec<VideoJob> = serde_json::from_value(video_jobs_json)
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        let result = ProcessingJobWithVideoJobResponse {
            id,
            organization_id,
            day_map_id,
            video_folder_path,
            status,
            created_at,
            video_jobs,
        };

        Ok(result)
    }

    pub async fn update_video_jobs_status(
        &self,
        video_job_id: &Uuid,
        new_status: ProcessingStatus,
    ) -> Result<VideoJob> {
        let result = sqlx::query_as(
            r#"
            UPDATE video_jobs
            SET status = $1
            WHERE id = $2
            RETURNING *
            "#,
        )
        .bind(new_status)
        .bind(video_job_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn get_video_job_by_id(&self, id: &Uuid) -> Result<Option<VideoJob>> {
        let result = sqlx::query_as(r#"SELECT * FROM video_jobs WHERE id = $1"#)
            .bind(&id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(result)
    }
}
