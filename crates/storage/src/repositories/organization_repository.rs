use crate::models::organization::{
    CameraPreset, CreateCameraPreset, CreateOrganization, Organization,
};
use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Pool, Postgres};
use uuid::Uuid;

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

    pub async fn get_organization_by_id(&self, id: Uuid) -> Result<Option<Organization>> {
        let result = sqlx::query_as(r#"SELECT * FROM organizations WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .context("Error to fetch organization")?;

        Ok(result)
    }

    pub async fn create_camera_preset(
        &self,
        organization_id: Uuid,
        camera: CreateCameraPreset,
    ) -> Result<CameraPreset> {
        let result = sqlx::query_as(
            r#"
            INSERT INTO camera_presets (id, organization_id, camera_name, location, detectors, regions, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        ).bind(
            &Uuid::new_v4(),
        ).bind(&organization_id)
            .bind(&camera.camera_name)
            .bind(&camera.location)
            .bind(&camera.detectors)
            .bind(&camera.regions)
            .bind(Utc::now())
            .fetch_one(&self.pool)
            .await?;

        Ok(result)
    }

    pub async fn list_organizations(&self) -> Result<Vec<Organization>> {
        let result = sqlx::query_as(r#"SELECT * FROM organizations"#)
            .fetch_all(&self.pool)
            .await
            .context("Error to fetch list of organizations")?;

        Ok(result)
    }

    pub async fn list_camera_presets(&self, organization_id: Uuid) -> Result<Vec<CameraPreset>> {
        let result = sqlx::query_as(r#"SELECT * FROM camera_presets WHERE organization_id = $1"#)
            .bind(organization_id)
            .fetch_all(&self.pool)
            .await
            .context("Error to fetch list of camera_presets")?;

        Ok(result)
    }
}
