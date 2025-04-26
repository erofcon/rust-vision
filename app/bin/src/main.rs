use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use anyhow::Result;
use api::handlers::{health, publish, video};
use common::config::ProjectConfig;
use queue::connection::MQ;
use std::sync::Arc;
use storage::database::Database;
use storage::repositories::video_repository::VideoRepository;

mod frame_processor;
mod handler;

#[actix_web::main]
async fn main() -> Result<()> {
    gst::init()?;
    let config = ProjectConfig::load()?;

    let db = Database::new(&config.database.connection_string()).await?;
    db.ping().await?;

    let db_pool = db.get_pool().clone();

    let mq = MQ::new(&config.mq.connection_to_string()).await?;
    let mq_channel = mq.get_channel().clone();

    println!("Spawning {} worker(s)...", &config.worker.worker_count);
    let video_repo = Arc::new(VideoRepository::new(db_pool.clone()));
    for id in 0..config.worker.worker_count {
        let chan = mq.get_channel().clone();
        let repo = Arc::clone(&video_repo);
        let model = Arc::new(config.base_detection_model.clone());

        actix_web::rt::spawn(async move {
            let _ = handler::spawn_worker(id, chan, repo, model).await;
        });
    }
    let bind_addr = format!("{}:{}", &config.api.host, &config.api.port);
    println!("Starting HTTP server on {}", bind_addr);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .app_data(web::Data::new(db_pool.clone()))
            .app_data(web::Data::new(mq_channel.clone()))
            .configure(health::config)
            .configure(video::config)
            .configure(publish::config)
    })
    .bind(&bind_addr)?
    .run()
    .await?;

    Ok(())
}
