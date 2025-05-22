use crate::error::ApiError;
use actix_multipart::Multipart;
use actix_web::{post, HttpResponse, Responder};
use anyhow::Result;

#[post("/api/v1/video_handler/upload")]
async fn upload_video(mut payload: Multipart) -> Result<impl Responder, ApiError> {



    Ok(HttpResponse::Created())
}
