use crate::models::day_map::{CreateDayMap, CreateDayMapEntry, DayMap, DayMapEntry};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};
use uuid::Uuid;

pub struct DayMapRepository {
    pool: Pool<Postgres>,
}

impl DayMapRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn create_day_map(&self, org_id: &Uuid, map: &CreateDayMap) -> Result<DayMap> {
        let result = sqlx::query_as(
            r#"
            INSERT INTO day_maps (id, organization_id, title, description, created_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
            "#,
        )
        .bind(&Uuid::new_v4())
        .bind(org_id)
        .bind(&map.title)
        .bind(&map.description)
        .bind(&Utc::now())
        .fetch_one(&self.pool)
        .await
        .context("Error to create day_map")?;

        Ok(result)
    }

    pub async fn get_day_map_by_id(&self, org_id: &Uuid) -> Result<Option<DayMap>> {
        let result = sqlx::query_as(r#"SELECT * FROM day_maps WHERE id = $1"#)
            .bind(org_id)
            .fetch_optional(&self.pool)
            .await
            .context("Error to fetch organization")?;

        Ok(result)
    }

    pub async fn create_day_map_entries(
        &self,
        day_map_id: &Uuid,
        map_entries: &Vec<CreateDayMapEntry>,
    ) -> Result<Vec<DayMapEntry>> {
        let record_ids: Vec<Uuid> = map_entries.iter().map(|_| Uuid::new_v4()).collect();
        let full_names: Vec<Option<String>> =
            map_entries.iter().map(|r| r.full_name.clone()).collect();
        let card_ids: Vec<&str> = map_entries.iter().map(|r| r.card_id.as_str()).collect();
        let entry_times: Vec<DateTime<Utc>> = map_entries.iter().map(|r| r.entry_time).collect();

        let created_at = Utc::now();

        let result = sqlx::query_as(
            r#"
            INSERT INTO day_map_entries (id, day_map_id, full_name, card_id, entry_time, created_at)
            SELECT t.id, $1::UUID, t.full_name, t.card_id, t.entry_time, $2::TIMESTAMPTZ
            FROM unnest($3::UUID[], $4::TEXT[], $5::TEXT[], $6::TIMESTAMPTZ[])
              AS t(id, full_name, card_id, entry_time)
            RETURNING *
        "#,
        )
        .bind(day_map_id)
        .bind(created_at)
        .bind(&record_ids)
        .bind(&full_names)
        .bind(&card_ids)
        .bind(&entry_times)
        .fetch_all(&self.pool)
        .await?;

        Ok(result)
    }
}
