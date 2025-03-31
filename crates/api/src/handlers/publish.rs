use crate::error::ApiError;
use crate::handlers::video::{get_video, upload_video, video_delete, video_list};
use actix_web::{post, web, HttpResponse, Responder};
use anyhow::{Context, Result};
use lapin::Channel;
use queue::producer::Producer;
use queue::utils::{Payload, QueueType};
use sqlx::{Pool, Postgres};
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
    let mq = mq.get_ref().clone();

    let _ = video_repo
        .get_by_id(video_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let payload = Payload { id: video_id };

    let bytes: Vec<u8> = bincode::serialize(&payload).context("Error to serialize payload")?;

    let producer = Producer::new(mq).context("Producer creation error")?;

    producer.publish(&bytes, QueueType::RunPipeline).await?;

    video_repo
        .change_status(video_id, &VideoStatus::Waiting)
        .await
        .context("Error to change video status")?;

    Ok(HttpResponse::Ok())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(publish_pipeline);
}
