use crate::models::video::Video;
use anyhow::{Context, Result};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

pub struct VideoRepository {
    pool: Pool<Postgres>,
}

impl VideoRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn create(&self, video: &Video) -> Result<Video> {
        let result = sqlx::query_as(
            r#"
            INSERT INTO videos (id, title, description, file_path, status)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(&video.id)
        .bind(&video.title)
        .bind(&video.description)
        .bind(&video.file_path)
        .bind(&video.status)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create video")?;

        Ok(result)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Video> {
        let result = sqlx::query_as(r#"SELECT * FROM videos WHERE id = $1"#)
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .context("Error receiving video")?;

        Ok(result)
    }
}
