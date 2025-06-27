// TODO: to delete

use crate::error::ApiError;
use crate::handlers::video::upload_video;
use crate::helper::read_field_data;
use actix_multipart::Multipart;
use actix_web::{post, web, HttpResponse, Responder};
use anyhow::Result;
use chrono::Utc;
use futures::{StreamExt, TryStreamExt};
use sqlx::{Pool, Postgres};
use storage::models::map_of_day::{AccessLog, EventStatus, MapOfDay};
use storage::repositories::map_of_day_repository::MapOfDayRepository;
use uuid::Uuid;

#[post("/api/v1/create/map_of_day")]
async fn create_map_of_day(
    pool: web::Data<Pool<Postgres>>,
    mut payload: Multipart,
) -> Result<impl Responder, ApiError> {
    let map_repo = MapOfDayRepository::new(pool.get_ref().clone());
    let mut description = None;
    let mut filename = String::new();
    let mut file_bytes = Vec::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition().ok_or(ApiError::BadRequest(
            "Bad request, invalid syntax".to_string(),
        ))?;

        if let Some(name) = content_disposition.get_name() {
            match name {
                "description" => {
                    let data = read_field_data(field).await?;

                    let desc_str = String::from_utf8_lossy(&data).to_string();
                    if !desc_str.is_empty() {
                        description = Some(desc_str);
                    }
                }
                "file" => {
                    if let Some(file_name) = content_disposition.get_filename() {
                        filename = file_name.to_string();

                        while let Some(chunk) = field.next().await {
                            file_bytes.extend_from_slice(&chunk.unwrap());
                        }
                    } else {
                        return Err(ApiError::BadRequest("File name not specified".to_string()));
                    }
                }
                _ => (),
            }
        }
    }

    let map_of_day = MapOfDay {
        id: Uuid::new_v4(),
        file_name: "test".to_string(),
        description,
        created_at: Utc::now(),
    };

    let result = map_repo.create_mod(&map_of_day).await?;

    let access_log = AccessLog {
        id: Uuid::new_v4(),
        full_name: "test access".to_string(),
        event_time: Utc::now(),
        event: EventStatus::Entrance,
        card_id: "23233232".to_string(),
        map_of_day_id: result.id,
    };

    let access = map_repo.create_access_log(&access_log).await?;

    // TODO: add xlsx parser

    Ok(HttpResponse::Ok().json(access))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(upload_video).service(create_map_of_day);
}
