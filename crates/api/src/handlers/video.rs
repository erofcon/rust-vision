use actix_multipart::Multipart;
use actix_web::{delete, get, post, web, HttpResponse, Responder};
use chrono::Utc;
use futures::TryStreamExt;
use sqlx::{Pool, Postgres};
use std::fs;
use std::path::Path;
use uuid::Uuid;

use crate::error::ApiError;
use crate::handlers::helper::{delete_file, read_field_data, save_file};
use crate::models::requests::{VideoListResponse, VideoResponse, VideoUploadResponse};
use storage::models::video::{Video, VideoStatus};
use storage::repositories::video_repository::VideoRepository;

#[post("/api/v1/video/upload")]
async fn upload_video(
    pool: web::Data<Pool<Postgres>>,
    mut payload: Multipart,
) -> Result<impl Responder, ApiError> {
    let video_repo = VideoRepository::new(pool.get_ref().clone());

    let video_id = Uuid::new_v4();
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "uploads".to_string());

    fs::create_dir_all(&upload_dir)?;

    let mut title = String::new();
    let mut description = None;
    let mut filename = String::new();
    let mut filepath = String::new();

    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_disposition = field.content_disposition().ok_or(ApiError::BadRequest(
            "Bad request, invalid syntax".to_string(),
        ))?;

        if let Some(name) = content_disposition.get_name() {
            match name {
                "title" => {
                    let data = read_field_data(field).await?;
                    title = String::from_utf8_lossy(&data).to_string();
                }
                "description" => {
                    let data = read_field_data(field).await?;

                    let desc_str = String::from_utf8_lossy(&data).to_string();
                    if !desc_str.is_empty() {
                        description = Some(desc_str);
                    }
                }
                "file" => {
                    if let Some(file_name) = content_disposition.get_filename() {
                        let safe_filename = sanitize_filename::sanitize(file_name);
                        let file_ext = Path::new(&safe_filename)
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .unwrap_or("mp4");

                        filename = format!("{}.{}", video_id, file_ext);
                        filepath = format!("{}/{}", upload_dir, filename);

                        save_file(field, &filepath).await?;
                    } else {
                        return Err(ApiError::BadRequest("File name not specified".to_string()));
                    }
                }

                _ => (),
            }
        }
    }

    if title.is_empty() {
        if !filepath.is_empty() {
            fs::remove_file(filepath).or_else(|e| Err(ApiError::BadRequest(e.to_string())))?;
        }

        return Err(ApiError::ValidationError(
            "Video title is required".to_string(),
        ));
    }

    if filename.is_empty() {
        return Err(ApiError::ValidationError(
            "Video file is required".to_string(),
        ));
    }

    let video = Video {
        id: video_id,
        title,
        description,
        file_path: format!("{}/{}", upload_dir, filename),
        created_at: Utc::now(),
        status: VideoStatus::Uploaded,
    };

    let result = video_repo.create(&video).await?;

    let response = VideoUploadResponse {
        id: result.id,
        title: result.title,
        status: format!("{:?}", result.status),
    };

    Ok(HttpResponse::Created().json(response))
}

#[get("/api/v1/video/{id}")]
async fn get_video(
    pool: web::Data<Pool<Postgres>>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let video_id = path.into_inner();
    let video_repo = VideoRepository::new(pool.get_ref().clone());

    let video = video_repo
        .get_by_id(video_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let response = VideoResponse {
        id: video.id,
        title: video.title,
        description: Option::from(video.description),
        status: format!("{:?}", video.status),
        created_at: video.created_at.to_rfc3339(),
    };

    Ok(HttpResponse::Ok().json(response))
}

#[post("/api/v1/video/list")]
async fn video_list(
    pool: web::Data<Pool<Postgres>>,
    // query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<impl Responder, ApiError> {
    // TODO: add filter

    // let status_str = query
    //     .get("status")
    //     .map(|s| s.as_str())
    //     .unwrap_or("Completed");
    //
    // let status = match status_str {
    //     "Uploaded" => VideoStatus::Uploaded,
    //     "Processing" => VideoStatus::Processing,
    //     "Completed" => VideoStatus::Completed,
    //     "Failed" => VideoStatus::Failed,
    //     _ => VideoStatus::Completed,
    // };
    // let videos = video_repo.list_by_status(status).await?;

    let video_repo = VideoRepository::new(pool.get_ref().clone());

    let video_list = video_repo.get_video_list().await?;

    let video_responses: Vec<VideoResponse> = video_list
        .into_iter()
        .map(|video| VideoResponse {
            id: video.id,
            title: video.title,
            description: video.description,
            status: format!("{:?}", video.status),
            created_at: video.created_at.to_rfc3339(),
        })
        .collect();

    let response = VideoListResponse {
        total: video_responses.len(),
        videos: video_responses,
    };

    Ok(HttpResponse::Ok().json(response))
}

#[delete("/api/v1/video/delete/{id}")]
async fn video_delete(
    pool: web::Data<Pool<Postgres>>,
    path: web::Path<Uuid>,
) -> Result<impl Responder, ApiError> {
    let id = path.into_inner();
    let video_repo = VideoRepository::new(pool.get_ref().clone());

    let video = video_repo.get_by_id(id).await?;

    if let Some(video) = video {
        delete_file(video.file_path).await?;
    } else {
        return Err(ApiError::NotFound);
    }

    let video = video_repo.delete_video(id).await?;

    Ok(HttpResponse::Ok().json(video))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(upload_video)
        .service(get_video)
        .service(video_list)
        .service(video_delete);
}
