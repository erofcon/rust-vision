use crate::models::organization::{
    CameraPreset, CameraPresetDb, CreateCameraPreset, CreateOrganization, Organization,
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::to_value;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

#[derive(Clone)]
pub struct OrganizationRepository {
    pool: Pool<Postgres>,
}

impl OrganizationRepository {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }

    pub async fn create_organization(&self, org: CreateOrganization) -> Result<Organization> {
        let result = sqlx::query_as(
            r#"
            INSERT INTO organizations (id, name, created_at)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(&Uuid::new_v4())
        .bind(&org.name)
        .bind(&Utc::now())
        .fetch_one(&self.pool)
        .await
        .context("Error to create camera organization")?;

        Ok(result)
    }

    pub async fn get_organization_by_id(&self, id: &Uuid) -> Result<Option<Organization>> {
        let result = sqlx::query_as(r#"SELECT * FROM organizations WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .context("Error to fetch organization")?;

        Ok(result)
    }

    pub async fn create_camera_preset(
        &self,
        organization_id: &Uuid,
        camera: CreateCameraPreset,
    ) -> Result<CameraPreset> {
        let detectors_json = to_value(&camera.detectors)?;
        let regions_json = to_value(&camera.regions)?;
        let now = Utc::now();
        let id = Uuid::new_v4();

        let record = sqlx::query_as::<_, CameraPresetDb>(
            r#"
        INSERT INTO camera_presets (id, organization_id, camera_name, location, detectors, regions, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
        )
            .bind(id)
            .bind(organization_id)
            .bind(&camera.camera_name)
            .bind(&camera.location)
            .bind(&detectors_json)
            .bind(&regions_json)
            .bind(now)
            .fetch_one(&self.pool)
            .await?;

        Ok(CameraPreset::try_from(record)?)
    }

    pub async fn list_organizations(&self) -> Result<Vec<Organization>> {
        let result = sqlx::query_as(r#"SELECT * FROM organizations"#)
            .fetch_all(&self.pool)
            .await
            .context("Error to fetch list of organizations")?;

        Ok(result)
    }

    pub async fn list_camera_presets(&self, organization_id: &Uuid) -> Result<Vec<CameraPreset>> {
        let records = sqlx::query_as::<_, CameraPresetDb>(
            r#"
        SELECT * FROM camera_presets
        WHERE organization_id = $1
        ORDER BY created_at DESC
        "#,
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?;

        let mut presets = Vec::with_capacity(records.len());
        for record in records {
            presets.push(CameraPreset::try_from(record)?);
        }

        Ok(presets)
    }
    pub async fn get_camera_preset_by_name(&self, name: &String) -> Result<Option<CameraPreset>> {
        let result = sqlx::query_as::<_, CameraPresetDb>(
            r#"
           SELECT * FROM camera_presets WHERE camera_name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        let mapped = match result {
            Some(record) => Some(CameraPreset::try_from(record)?),
            None => None,
        };

        Ok(mapped)
    }

    pub async fn get_camera_preset_by_id(&self, id: &Uuid) -> Result<Option<CameraPreset>> {
        let result = sqlx::query_as::<_, CameraPresetDb>(
            r#"
        SELECT * FROM camera_presets WHERE id = $1
        "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        // Преобразуем к финальной структуре
        let mapped = match result {
            Some(record) => Some(CameraPreset::try_from(record)?),
            None => None,
        };

        Ok(mapped)
    }
}
