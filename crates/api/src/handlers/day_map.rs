use crate::error::ApiError;
use actix_multipart::Multipart;
use actix_web::{post, web, HttpResponse, Responder};
use anyhow::Result;
use chrono::Utc;
use futures::StreamExt;
use sqlx::{Pool, Postgres};
use storage::models::day_map::{CreateDayMap, CreateDayMapEntry};
use storage::repositories::day_map_repository::DayMapRepository;
use storage::repositories::organization_repository::OrganizationRepository;
use uuid::Uuid;

#[post("/api/v1/create/{org_id}/day_map")]
async fn create_day_map(
    pool: web::Data<Pool<Postgres>>,
    path: web::Path<Uuid>,
    item: web::Json<CreateDayMap>,
) -> Result<impl Responder, ApiError> {
    let org_repo = OrganizationRepository::new(pool.get_ref().clone());

    let org = org_repo
        .get_organization_by_id(path.into_inner())
        .await?
        .ok_or(ApiError::NotFound)?;

    let day_map_repo = DayMapRepository::new(pool.get_ref().clone());

    let result = day_map_repo
        .create_day_map(&org.id, &item.into_inner())
        .await?;

    Ok(HttpResponse::Created().json(result))
}

#[post("/api/v1/create/day_map/{day_map_id}/entries")]
async fn create_day_map_entries(
    pool: web::Data<Pool<Postgres>>,
    path: web::Path<Uuid>,
    mut payload: Multipart,
) -> Result<impl Responder, ApiError> {
    let day_map_repo = DayMapRepository::new(pool.get_ref().clone());

    let day_map = day_map_repo
        .get_day_map_by_id(&path.into_inner())
        .await?
        .ok_or(ApiError::NotFound)?;

    // while let Some(item) = payload.next().await {
    //     // let mut field = item;
    //     // while let Some(_chunk) = field.next().await {
    //     //     // TODO: add xlsx pars
    //     // }
    // }

    let simulate_records = vec![
        CreateDayMapEntry {
            full_name: Some("Иван1 Иванов".to_string()),
            card_id: "card45ewre6".to_string(),
            entry_time: Utc::now(),
        },
        CreateDayMapEntry {
            full_name: Some("Иван2 Иванов".to_string()),
            card_id: "card4wer2356".to_string(),
            entry_time: Utc::now(),
        },
        CreateDayMapEntry {
            full_name: Some("Иван3 Иванов".to_string()),
            card_id: "card4erww56".to_string(),
            entry_time: Utc::now(),
        },
    ];

    let result = day_map_repo
        .create_day_map_entries(&day_map.id, &simulate_records)
        .await?;

    Ok(HttpResponse::Created().json(result))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_day_map).service(create_day_map_entries);
}
