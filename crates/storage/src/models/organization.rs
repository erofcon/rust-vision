use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateOrganization {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct CameraPreset {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub camera_name: String,
    pub location: String,
    pub detectors: serde_json::Value,
    pub regions: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateCameraPreset {
    pub camera_name: String,
    pub location: String,
    pub detectors: serde_json::Value, //  [ "face", "person" ]
    pub regions: serde_json::Value,   // { "face": { x, y, width, height }, "person": { ... } }
}
