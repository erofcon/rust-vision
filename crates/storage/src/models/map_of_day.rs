use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct MapOfDay {
    pub id: Uuid,
    pub file_name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[sqlx(type_name = "event_status")]
#[sqlx(rename_all = "lowercase")]
pub enum EventStatus {
    Entrance,
    Exit,
}

#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct AccessLog {
    pub id: Uuid,
    pub full_name: String,
    pub event_time: DateTime<Utc>,
    pub event: EventStatus,
    pub card_id: String,
    pub map_of_day_id: Uuid,
}
