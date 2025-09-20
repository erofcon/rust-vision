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
use storage::repositories::processing_jobs_repository::ProcessingJobsRepository;
use uuid::Uuid;

#[post("/api/v1/process/publish/{id}")]
async fn process_publish(
    pool: web::Data<Pool<Postgres>>,
    mq: web::Data<Channel>,
    processing_job_id: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let process = ProcessingJobsRepository::new(pool.get_ref().clone());
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
                let payload = Payload {
                    video_job_id: video_job.id,
                };
                let bytes = payload
                    .serialize()
                    .context("Failed to serialize payload")
                    .map_err(ApiError::from)?;

                producer
                    .publish(&bytes, QueueType::VideoProcessing)
                    .await
                    .context("Error to publish producer")
                    .map_err(ApiError::from)?;

                process
                    .update_video_jobs_status(&video_job.id, ProcessingStatus::Waiting)
                    .await?;
            }
            _ => {
                // (Waiting, Processing, Completed)
            }
        }
    }

    Ok(HttpResponse::Ok().json(result))
}

#[post("/api/v1/process/video/cancel/{id}")]
async fn pipeline_cancel(
    pool: web::Data<Pool<Postgres>>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let video_id = path.into_inner();
    let process = ProcessingJobsRepository::new(pool.get_ref().clone());

    let video = process
        .get_video_job_by_id(&video_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    if video.status != ProcessingStatus::Waiting && video.status != ProcessingStatus::Processing {
        return Err(ApiError::BadRequest(
            "Video processing has already been cancelled".into(),
        ));
    }

    if video.status == ProcessingStatus::Processing {
        if stop_video_processing(&video_id) {
            println!("Request to stop video processing sent {}", &video_id);
        } else {
            println!("Could not find active processing for video {}", &video_id);
        }
    }

    process
        .update_video_jobs_status(&video_id, ProcessingStatus::Cancelled)
        .await
        .map_err(ApiError::from)?;

    Ok(HttpResponse::Ok())
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(pipeline_cancel).service(process_publish);
}
