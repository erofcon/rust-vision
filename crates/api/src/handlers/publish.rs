use crate::error::ApiError;
use actix_web::{post, web, HttpResponse, Responder};
use anyhow::{Context, Result};
use common::pipeline_registry::stop_video_processing;
use lapin::Channel;
use queue::producer::Producer;
use queue::utils::Payload;
use queue::utils::QueueType;
use sqlx::{Pool, Postgres};
use std::sync::atomic::Ordering;
use storage::models::video::VideoStatus;
use storage::repositories::video_repository::VideoRepository;
use uuid::Uuid;

#[post("/api/v1/video/pipeline/publish/{id}")]
async fn publish_pipeline(
    pool: web::Data<Pool<Postgres>>,
    mq: web::Data<Channel>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let video_id = path.into_inner();
    let video_repo = VideoRepository::new(pool.get_ref().clone());

    let video = video_repo
        .get_by_id(video_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    if video.status == VideoStatus::Waiting || video.status == VideoStatus::Processing {
        return Err(ApiError::BadRequest(
            "The video is already in processing or pending".into(),
        ));
    }

    let payload = Payload { id: video.id };

    let bytes = payload
        .serialize()
        .context("Failed to serialize payload")
        .map_err(ApiError::from)?;

    let producer = Producer::new(mq.get_ref().clone()).context("Producer creation error")?;
    producer
        .publish(&bytes, QueueType::VideoProcessing)
        .await
        .context("Error to publish producer")
        .map_err(ApiError::from)?;

    video_repo
        .change_status(video_id, &VideoStatus::Waiting)
        .await
        .context("Error to change video status")
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok())
}

#[post("/api/v1/video/pipeline/cancel/{id}")]
async fn pipeline_cancel(
    pool: web::Data<Pool<Postgres>>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let video_id = path.into_inner();
    let video_repo = VideoRepository::new(pool.get_ref().clone());

    let video = video_repo
        .get_by_id(video_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    if video.status != VideoStatus::Waiting && video.status != VideoStatus::Processing {
        return Err(ApiError::BadRequest(
            "Video processing has already been cancelled".into(),
        ));
    }

    // Если видео в обработке, пытаемся остановить его через менеджер
    if video.status == VideoStatus::Processing {
        if stop_video_processing(&video_id) {
            println!("Отправлен запрос на остановку обработки видео {}", video_id);
        } else {
            println!("Не удалось найти активную обработку для видео {}", video_id);
        }
    }

    // В любом случае обновляем статус в БД
    video_repo
        .change_status(video_id, &VideoStatus::Cancelled)
        .await
        .context("Error to change video status")
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok())
}
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(publish_pipeline).service(pipeline_cancel);
}
