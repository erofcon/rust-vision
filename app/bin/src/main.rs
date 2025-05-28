use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use anyhow::Result;
use api::handlers::{day_map, health, organization, processing_job, publish};
use common::config::ProjectConfig;
use common::detection_state::DetectionState;
use detection::model::Model;
use futures::future::BoxFuture;
use gst::prelude::ElementExt;
use gst_streaming::pipeline::GstPipeline;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use motion::motion::Motion;
use queue::connection::MQ;
use queue::consumer::Consumer;
use queue::utils::{Payload, QueueType};
use sqlx::{Pool, Postgres};
use std::sync::{Arc, Mutex, RwLock};
use storage::database::Database;
use storage::repositories::processing_jobs_repository::ProcessingJobsRepository;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

#[actix_web::main]
async fn main() -> Result<()> {
    let config = ProjectConfig::load()?;

    gst::init()?;

    let model = Arc::new(RwLock::new(Model::new(&config.base_detection_model.path)?));
    println!("Model loaded and ready for shared access");

    // DB
    let db = Database::new(&config.database.connection_string()).await?;
    db.ping().await?;
    let db_pool = db.get_pool().clone();

    // MQ
    let mq = MQ::new(&config.mq.connection_to_string()).await?;
    let mq_channel = mq.get_channel().clone();

    // Workers
    println!("Spawning {} worker(s)...", config.worker.worker_count);
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    for id in 0..config.worker.worker_count {
        let channel = mq_channel.clone();
        let db_pool_clone = db_pool.clone();
        let model_clone = model.clone(); // Клонируем Arc<RwLock<Model>>
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            let handle = spawn_worker(id, channel, db_pool_clone, model_clone).await;
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

    let api_db = db.get_pool().clone();

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
            .app_data(web::Data::new(api_db.clone()))
            .app_data(web::Data::new(mq_channel.clone()))
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

pub async fn spawn_worker(
    worker_id: usize,
    channel: lapin::Channel,
    db: Pool<Postgres>,
    model: Arc<RwLock<Model>>, // RwLock для множественного чтения
) -> JoinHandle<()> {
    let mut consumer = Consumer::new(channel, QueueType::VideoProcessing)
        .await
        .expect("Failed to create consumer");

    let processing_job_repo = ProcessingJobsRepository::new(db);

    consumer.set_handler(move |data, _routing| {
        let processing_job_repo_clone = processing_job_repo.clone();
        let model_clone = model.clone();
        handle_message(
            data.to_vec(),
            processing_job_repo_clone,
            model_clone,
            worker_id,
        )
    });

    tokio::spawn(async move {
        if let Err(e) = consumer.start().await {
            eprintln!("[Worker {}] Consumer error: {}", worker_id, e);
        }
    })
}

fn handle_message(
    data: Vec<u8>,
    processing_job_repo: ProcessingJobsRepository,
    model: Arc<RwLock<Model>>,
    worker_id: usize,
) -> BoxFuture<'static, std::result::Result<(), anyhow::Error>> {
    Box::pin(async move {
        let payload = Payload::deserialize(&data)?;

        let video_job = processing_job_repo.get_video_job_by_id(&payload.id).await?;
        println!(
            "Processing video job: {} (file: {})",
            video_job.id, video_job.file_path
        );

        let rtmp_url = format!("rtmp://localhost/live/stream_{}", video_job.id);

        let motion = Arc::new(Mutex::new(Motion::new()?));

        let detection_state = Arc::new(Mutex::new(DetectionState::new(motion, model, 120, 30)));

        let detection_state_clone = detection_state.clone();

        let mut pipeline = GstPipeline::new(&video_job.file_path, &rtmp_url, move |buffer| {
            if let Err(e) = process_buffer(buffer, &detection_state_clone) {
                eprintln!("Error processing buffer: {}", e);
            }
        })?;

        println!("Starting pipeline for video job: {}", video_job.id);

        println!(
            "Worker {} processing video job: {} (file: {})",
            worker_id, video_job.id, video_job.file_path
        );

        tokio::task::spawn_blocking(move || pipeline.run())
            .await
            .map_err(|e| anyhow::anyhow!("Pipeline task panicked: {:?}", e))??;

        Ok(())
    })
}

fn process_buffer(buffer: &VideoFrame<Readable>, state: &Arc<Mutex<DetectionState>>) -> Result<()> {
    match state.lock() {
        Ok(mut state_guard) => {
            let _detection_result = state_guard.process_frame(buffer)?;
            // TODO: обработать результат детекции
            // Например: сохранить в БД, отправить уведомление и т.д.
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Failed to lock detection state mutex: {}", e);
        }
    }
}
