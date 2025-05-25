// TODO: change file path to services

use crate::error::ApiError;
use actix_multipart::Field;
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use futures::StreamExt;
use regex::Regex;
use std::io::Write;

pub async fn read_field_data(mut field: Field) -> Result<Vec<u8>, ApiError> {
    let mut data = Vec::new();

    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|e| ApiError::BadRequest(e.to_string()))?;
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

pub async fn save_file(mut field: Field, filepath: &str) -> Result<(), ApiError> {
    let mut file = std::fs::File::create(filepath)?;
    while let Some(chunk) = field.next().await {
        let chunk = chunk.map_err(|e| ApiError::BadRequest(e.to_string()))?;
        file.write_all(&chunk)?;
    }
    Ok(())
}

pub async fn delete_file(path: String) -> Result<()> {
    std::fs::remove_file(path)?;
    Ok(())
}

pub fn parse_video_details(video_name: &str) -> Result<(DateTime<Utc>, String)> {
    let re = Regex::new(
        r"^NVR_(?P<channel>ch\d+)_main_(?P<start>\d{14})_(?P<end>\d{14})\.(?P<ext>mp4|avi)$",
    )
    .context("Invalid regex pattern for parsing video details")?;
    let caps = re.captures(video_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Filename format not recognized or missing time stamps: {}",
            video_name
        )
    })?;
    let start_str = caps
        .name("start")
        .ok_or_else(|| anyhow::anyhow!("Start time not found in filename: {}", video_name))?
        .as_str();
    let channel = caps
        .name("channel")
        .ok_or_else(|| anyhow::anyhow!("Camera channel not found in filename: {}", video_name))?
        .as_str()
        .to_string();
    let naive = NaiveDateTime::parse_from_str(start_str, "%Y%m%d%H%M%S")
        .context("Failed to parse start time from filename")?;
    let start_time = DateTime::<Utc>::from_utc(naive, Utc);
    Ok((start_time, channel))
}
