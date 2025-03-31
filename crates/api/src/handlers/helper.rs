use crate::error::ApiError;
use actix_multipart::Field;
use futures::StreamExt;
use std::io::Write;

use anyhow::Result;

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
