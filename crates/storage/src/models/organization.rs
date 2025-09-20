use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

#[derive(Debug, Deserialize, sqlx::FromRow)]
pub struct CameraPresetDb {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub camera_name: String,
    pub location: String,
    pub detectors: serde_json::Value,
    pub regions: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectorType {
    Face,
    Person,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub x1: u32,
    pub y1: u32,
    pub x2: u32,
    pub y2: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CameraPreset {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub camera_name: String,
    pub location: String,
    pub detectors: Vec<DetectorType>,
    pub regions: HashMap<DetectorType, Region>, // [ "face", "person" ]
    pub created_at: DateTime<Utc>,              // { "face": { x1, y1, x2, y2 }, "person": { ... } }
}

impl TryFrom<CameraPresetDb> for CameraPreset {
    type Error = serde_json::Error;

    fn try_from(db: CameraPresetDb) -> Result<Self, Self::Error> {
        Ok(Self {
            id: db.id,
            organization_id: db.organization_id,
            camera_name: db.camera_name,
            location: db.location,
            created_at: db.created_at,
            detectors: serde_json::from_value(db.detectors)?,
            regions: serde_json::from_value(db.regions)?,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCameraPreset {
    pub organization_id: Uuid,
    pub camera_name: String,
    pub location: String,
    pub detectors: Vec<DetectorType>, // [ "face", "person" ]
    pub regions: HashMap<DetectorType, Region>, // { "face": { x1, y1, x2, y2 }, "person": { ... } }
}
