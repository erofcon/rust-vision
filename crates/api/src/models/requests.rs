use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct MapOfDay {
    pub id: Uuid,
    pub file_name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MapOfDayListResponse {
    pub map_of_days: Vec<MapOfDay>,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessLog {
    pub id: Uuid,
    pub full_name: String,
    pub event_time: String,
    pub event: String,
    pub card_id: String,
    pub map_of_day_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessLogListResponse {
    pub map_of_days: Vec<AccessLog>,
    pub total: usize,
}

// outdated code

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoUploadResponse {
    pub id: Uuid,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoListResponse {
    pub videos: Vec<VideoResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}
