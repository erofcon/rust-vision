use crate::error::ApiError;
use actix_web::{post, web, HttpResponse, Responder};
use anyhow::Result;
use sqlx::{Pool, Postgres};
use storage::models::organization::{CreateCameraPreset, CreateOrganization};
use storage::repositories::organization_repository::OrganizationRepository;

#[post("/api/v1/create/organization")]
async fn create_organization(
    pool: web::Data<Pool<Postgres>>,
    item: web::Json<CreateOrganization>,
) -> Result<impl Responder, ApiError> {
    let org_repo = OrganizationRepository::new(pool.get_ref().clone());

    let result = org_repo.create_organization(item.into_inner()).await?;

    Ok(HttpResponse::Created().json(result))
}

#[post("/api/v1/create/camera_preset")]
async fn create_camera_preset(
    pool: web::Data<Pool<Postgres>>,
    item: web::Json<CreateCameraPreset>,
) -> Result<impl Responder, ApiError> {
    /*
    TODO: add JSON struct checker
    {
        "camera_name": "chr_1",
        "location": "предкладовая",
        "detectors": ["face", "person"],
        "regions": {
            "face": {
                "x": 12,
                "y":90,
                "w": 23,
                "h": 98
            }
        }
    }
     */

    let org_repo = OrganizationRepository::new(pool.get_ref().clone());

    let org = org_repo
        .get_organization_by_id(&item.organization_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let result = org_repo
        .create_camera_preset(&org.id, item.into_inner())
        .await?;

    Ok(HttpResponse::Created().json(result))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(create_organization)
        .service(create_camera_preset);
}
