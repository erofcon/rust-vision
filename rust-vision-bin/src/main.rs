mod task_handler;

use crate::task_handler::spawn_worker;
use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use anyhow::Result;
use api::handlers::{day_map, health, organization, processing_job, publish};
use inference_core::globals::{
    FACE_DETECTION_MODEL, FACE_RECOGNITION_MODEL, OBJECT_DETECTION_MODEL,
};
use once_cell::sync::Lazy;
use project_config::global::PROJECT_CONFIG;
use queue::global::get_mq;
use storage::global::get_database;
use tokio::sync::broadcast;

#[actix_web::main]
async fn main() -> Result<()> {
    gst::init()?;

    let database = get_database().await.get_pool();
    let mq = get_mq().await.get_channel();

    Lazy::force(&FACE_DETECTION_MODEL);
    Lazy::force(&FACE_RECOGNITION_MODEL);
    Lazy::force(&OBJECT_DETECTION_MODEL);

    //
    // WORKERS
    //

    let worker_count = &PROJECT_CONFIG.worker.worker_count;
    println!("Spawning {} worker(s)...", worker_count);

    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    for id in 0..*worker_count {
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let handle = spawn_worker(id).await;

            tokio::select! {
                _ = shutdown_rx.recv() => {
                    println!("Worker {} received shutdown signal", id);
                }
                res = handle => {
                    if let Err(e) = res {
                        eprintln!("Worker {} panicked: {:?}", id, e);
                    }
                }
            }
            println!("Worker {} stopped", id);
        });
    }

    //
    // API SERVER
    //

    let bind_addr = format!("{}:{}", &PROJECT_CONFIG.api.host, PROJECT_CONFIG.api.port);
    println!("Starting HTTP server on {}", bind_addr);

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .app_data(web::Data::new(database.clone()))
            .app_data(web::Data::new(mq.clone()))
            .configure(health::config)
            .configure(publish::config)
            .configure(organization::config)
            .configure(day_map::config)
            .configure(processing_job::config)
    })
    .bind(&bind_addr)?
    .run();

    tokio::select! {
        res = server => res.map_err(|e| anyhow::anyhow!(e)),
        _ = tokio::signal::ctrl_c() => {
            println!("Shutdown signal received in main");
            Ok(())
        }
    }
    .map(|_| ())?;

    Ok(())
}
