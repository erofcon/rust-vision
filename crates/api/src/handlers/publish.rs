use crate::error::ApiError;
use actix_web::{post, web, HttpResponse, Responder};
use anyhow::{Context, Result};
use common::pipeline_registry::stop_video_processing;
use lapin::Channel;
use queue::producer::Producer;
use queue::utils::Payload;
use queue::utils::QueueType;
use sqlx::{Pool, Postgres};
use storage::models::processing_job::ProcessingStatus;
use storage::models::video::VideoStatus;
use storage::repositories::processing_jobs_repository::ProcessingJobsRepository;
use storage::repositories::video_repository::VideoRepository;
use uuid::Uuid;

#[post("/api/v1/process/publish/{id}")]
async fn process_publish(
    pool: web::Data<Pool<Postgres>>,
    mq: web::Data<Channel>,
    processing_job_id: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let pool = pool.get_ref().clone();
    let process = ProcessingJobsRepository::new(pool.clone());
    let producer = Producer::new(mq.get_ref().clone()).context("Producer creation error")?;

    let result = process
        .get_processing_job_with_video_job(&processing_job_id)
        .await?;

    for video_job in &result.video_jobs {
        match video_job.status {
            ProcessingStatus::Uploaded
            | ProcessingStatus::Failed
            | ProcessingStatus::Cancelled
            | ProcessingStatus::Completed => {
                let payload = Payload { id: video_job.id };
                let bytes = payload
                    .serialize()
                    .context("Failed to serialize payload")
                    .map_err(ApiError::from)?;

                producer
                    .publish(&bytes, QueueType::VideoProcessing)
                    .await
                    .context("Error to publish producer")
                    .map_err(ApiError::from)?;

                // process
                //     .update_video_job_status(&video_job.id, ProcessingStatus::Waiting)
                //     .await?;
            }
            _ => {
                // Не обновляем статус для других состояний (Waiting, Processing, Completed)
            }
        }
    }

    Ok(HttpResponse::Ok().json(result))
}

#[post("/api/v1/video/pipeline/publish/{id}")]
async fn publish_pipeline(
    pool: web::Data<Pool<Postgres>>,
    mq: web::Data<Channel>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    // TODO: to delete

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
    // TODO: to delete

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

    if video.status == VideoStatus::Processing {
        if stop_video_processing(&video_id) {
            println!("Request to stop video processing sent {}", video_id);
        } else {
            println!("Could not find active processing for video {}", video_id);
        }
    }

    video_repo
        .change_status(video_id, &VideoStatus::Cancelled)
        .await
        .context("Error to change video status")
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(publish_pipeline)
        .service(pipeline_cancel)
        .service(process_publish);
}
