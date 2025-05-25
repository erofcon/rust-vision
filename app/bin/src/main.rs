use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use anyhow::Result;
use api::handlers::{day_map, health, map_of_day, organization, processing_job, publish, video};
use common::config::ProjectConfig;
use detection::model::Model;
use queue::connection::MQ;
use std::sync::Arc;
use storage::database::Database;
use storage::repositories::video_repository::VideoRepository;
use tokio::sync::broadcast;

mod frame_processor;
mod handler;

#[actix_web::main]
async fn main() -> Result<()> {
    // Initialize GStreamer
    // TODO: delete to production

    unsafe {
        std::env::set_var("GST_DEBUG", "3");
        std::env::set_var("RUST_BACKTRACE", "full");
    }

    // gst::init()?;

    let config = ProjectConfig::load()?;

    // let model = Arc::new(Model::new(&config.base_detection_model.path)?);
    // println!(
    //     "Loaded detection model from {}",
    //     config.base_detection_model.path
    // );

    let db = Database::new(&config.database.connection_string()).await?;
    db.ping().await?;
    let db_pool = db.get_pool().clone();

    let mq = MQ::new(&config.mq.connection_to_string()).await?;
    let mq_channel = mq.get_channel().clone();

    // let video_repo = Arc::new(VideoRepository::new(db_pool.clone()));
    //
    // let (in_w, in_h) = (
    //     config.base_detection_model.input_width,
    //     config.base_detection_model.input_height,
    // );
    //
    // let frame_processor = frame_processor::build_frame_processor(Arc::clone(&model), in_w, in_h);
    //
    // println!("Spawning {} worker(s)...", config.worker.worker_count);
    // let (shutdown_tx, _) = broadcast::channel::<()>(1);
    //
    // for id in 0..config.worker.worker_count {
    //     let mut shutdown_rx = shutdown_tx.subscribe();
    //     let repo = Arc::clone(&video_repo);
    //     let chan = mq_channel.clone();
    //     let processor = Arc::clone(&frame_processor);
    //
    //     tokio::spawn(async move {
    //         Spawn consumer and return its JoinHandle
    // let handle = handler::spawn_worker(id, chan, repo, processor, in_w, in_h).await;
    //
    // tokio::select! {
    //     _ = shutdown_rx.recv() => {
    //         println!("Worker {} received shutdown signal", id);
    //     }
    //     res = handle => {
    //         if let Err(e) = res {
    //             eprintln!("Worker {} panicked: {:?}", id, e);
    //         }
    //     }
    // }
    //
    // println!("Worker {} stopped", id);
    // });
    // }

    // Start HTTP server
    let bind_addr = format!("{}:{}", config.api.host, config.api.port);
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
            .app_data(web::Data::new(db_pool.clone()))
            .app_data(web::Data::new(mq_channel.clone()))
            .configure(health::config)
            .configure(video::config)
            .configure(publish::config)
            .configure(map_of_day::config)
            .configure(organization::config)
            .configure(day_map::config)
            .configure(processing_job::config)
    })
    .bind(&bind_addr)?
    .run();

    // Graceful shutdown
    tokio::select! {
        res = server => res.map_err(|e| anyhow::anyhow!(e)),
        _ = tokio::signal::ctrl_c() => {
            println!("Shutdown signal received in main");
            Ok(())
        }
    }
    .map(|_| ())
}
