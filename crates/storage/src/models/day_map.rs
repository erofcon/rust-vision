use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct DayMap {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateDayMap {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, sqlx::FromRow)]
pub struct DayMapEntry {
    pub id: Uuid,
    pub day_map_id: Uuid,
    pub full_name: Option<String>,
    pub card_id: String,
    pub entry_time: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct CreateDayMapEntry {
    pub full_name: Option<String>,
    pub card_id: String,
    pub entry_time: DateTime<Utc>,
}
