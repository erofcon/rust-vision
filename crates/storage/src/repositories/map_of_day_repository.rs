use crate::models::map_of_day::{AccessLog, MapOfDay};
use anyhow::{Context, Result};
use sqlx::{Pool, Postgres};

pub struct MapOfDayRepository {
    pool: Pool<Postgres>,
}

impl MapOfDayRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn create_mod(&self, map_of_day: &MapOfDay) -> Result<MapOfDay> {
        let result = sqlx::query_as(
            r#"
            INSERT INTO map_of_day (id, file_name, description, created_at)
            VALUES ($1, $2, $3, $4)
            RETURNING *
            "#,
        )
        .bind(&map_of_day.id)
        .bind(&map_of_day.file_name)
        .bind(&map_of_day.description)
        .bind(&map_of_day.created_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn create_access_log(&self, access_log: &AccessLog) -> Result<AccessLog> {
        let result = sqlx::query_as(
            r#"
            INSERT INTO access_log (id, full_name, event_time, event, card_id, map_of_day_id)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(&access_log.id)
        .bind(&access_log.full_name)
        .bind(&access_log.event_time)
        .bind(&access_log.event)
        .bind(&access_log.card_id)
        .bind(&access_log.map_of_day_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to create access_log")?;

        Ok(result)
    }
}
