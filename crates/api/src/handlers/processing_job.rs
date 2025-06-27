use crate::error::ApiError;
use crate::helper::parse_video_details;
use crate::models::requests::ProcessingJobResponse;
use actix_web::{post, web, HttpResponse, Responder};
use anyhow::{Context, Result};
use sqlx::{Pool, Postgres};
use std::fmt::Debug;
use std::path::Path;
use storage::models::processing_job::CreateProcessingJob;
use storage::models::video_job::VideoJobCreate;
use storage::repositories::day_map_repository::DayMapRepository;
use storage::repositories::organization_repository::OrganizationRepository;
use storage::repositories::processing_jobs_repository::ProcessingJobsRepository;
use tokio::fs;

#[post("/api/v1/create/process")]
async fn create_processing_job(
    pool: web::Data<Pool<Postgres>>,
    item: web::Json<CreateProcessingJob>,
) -> Result<impl Responder, ApiError> {
    let mut created_video_jobs = Vec::new();

    let pool = pool.get_ref().clone();

    let (org_result, day_map_result) = futures::join!(
        async {
            let org_repo = OrganizationRepository::new(pool.clone());
            org_repo
                .get_organization_by_id(&item.organization_id)
                .await?
                .ok_or(ApiError::NotFound)
        },
        async {
            let day_map_repo = DayMapRepository::new(pool.clone());
            day_map_repo
                .get_day_map_by_id(&item.day_map_id)
                .await?
                .ok_or(ApiError::NotFound)
        }
    );

    let _org = org_result?;
    let _day_map = day_map_result?;

    let process = ProcessingJobsRepository::new(pool.clone());
    let process_result = process.create_processing_jobs(&item).await?;

    let video_folder_path = Path::new(&item.video_folder_path);
    if !video_folder_path.exists() {
        return Err(ApiError::BadRequest(format!(
            "Video folder does not exist: {}",
            item.video_folder_path
        )));
    }

    let canonical_video_folder_path = video_folder_path.canonicalize().with_context(|| {
        format!(
            "Failed to canonicalize video folder path: {}",
            item.video_folder_path
        )
    })?;

    let dir = fs::read_dir(video_folder_path).await.with_context(|| {
        format!(
            "Failed to read video folder directory: {}",
            item.video_folder_path
        )
    })?;

    let mut video_jobs = Vec::new();
    let mut dir_stream = Box::pin(dir);

    while let Some(entry_result) = dir_stream.next_entry().await? {
        let entry = entry_result;
        let path = entry.path();

        let canonical_file_path = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                println!(
                    "Failed to canonicalize file path {}: {:?}",
                    path.display(),
                    e
                );
                continue;
            }
        };

        if !canonical_file_path.starts_with(&canonical_video_folder_path) {
            println!(
                "File {} is outside the video folder, skipping",
                path.display()
            );
            continue;
        }

        let file_name = entry.file_name();
        let file_name_str = match file_name.to_str() {
            Some(s) => s,
            None => {
                println!("File name contains invalid UTF-8: {}", path.display());
                continue;
            }
        };

        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            let ext_lc = ext.to_lowercase();
            if ext_lc == "mp4" || ext_lc == "avi" {
                let video_name = file_name_str.to_string();
                let file_path = path.to_string_lossy().to_string();

                let (start_time, channel) =
                    parse_video_details(&video_name).with_context(|| {
                        format!("Failed to parse video details from name: {}", video_name)
                    })?;

                let org_repo = OrganizationRepository::new(pool.clone());
                let camera_preset = org_repo
                    .get_camera_preset_by_name(&channel)
                    .await?
                    .ok_or(ApiError::NotFound)?;

                let video_job = VideoJobCreate {
                    processing_jobs_id: process_result.id,
                    camera_presets_id: camera_preset.id,
                    video_name,
                    file_path,
                    start_time,
                };

                video_jobs.push(video_job);
            }
        }
    }

    for video_job in video_jobs {
        let created = process.create_video_job(&video_job).await?;
        created_video_jobs.push(created);
    }

    Ok(HttpResponse::Created().json(ProcessingJobResponse {
        processing_job: process_result,
        video_jobs: created_video_jobs,
    }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_processing_job);
}
