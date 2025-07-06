use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use anyhow::{Result, anyhow};
use api::handlers::{day_map, health, organization, processing_job, publish};
use common::config::ProjectConfig;
use common::detection_state::DetectionState;
use common::ort_session::Model;
use common::pipeline_registry::{register_video_processing, unregister_video_processing};
use common::utils::{Detectors, FaceModels};
use detection::utils::BoundingBox;
use futures::future::BoxFuture;
use gst_streaming::pipeline::GstPipeline;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use motion::motion::Motion;
use parking_lot::{Mutex, RwLock};
use queue::connection::MQ;
use queue::consumer::Consumer;
use queue::utils::{Payload, QueueType};
use sqlx::{Pool, Postgres};
use std::path::Path;
use std::sync::Arc;
use storage::database::Database;
use storage::models::organization::DetectorType;
use storage::models::processing_job::ProcessingStatus;
use storage::repositories::organization_repository::OrganizationRepository;
use storage::repositories::processing_jobs_repository::ProcessingJobsRepository;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use vision::face_database::FaceDatabase;

#[actix_web::main]
async fn main() -> Result<()> {
    let config = ProjectConfig::load()?;

    gst::init()?;

    let face_db_file = &config.face_database.db_file;
    let face_db = &config.face_database.path;

    let face_database = if Path::new(face_db_file).exists() {
        println!("Loading sources from the file...");
        Arc::new(RwLock::new(FaceDatabase::load_from_file(face_db)?))
    } else {
        println!("We build a database from folders...");
        let mut db = FaceDatabase::new();

        db.load_from_folder().await?;
        db.save_to_file(face_db_file)?;

        Arc::new(RwLock::new(db))
    };

    // models
    let object_detection_model = Arc::new(RwLock::new(Model::new(
        &config.object_detection_model.path,
    )?));

    let face_detection_model =
        Arc::new(RwLock::new(Model::new(&config.face_detection_model.path)?));

    let face_recognition_model = Arc::new(RwLock::new(Model::new(
        &config.face_recognition_model.path,
    )?));

    println!("Models loaded and ready for shared access");

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
        let object_detection_model_clone = object_detection_model.clone();
        let face_detection_model_clone = face_detection_model.clone();
        let face_recognition_model_clone = face_recognition_model.clone();
        let face_database_clone = face_database.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            let handle = spawn_worker(
                id,
                channel,
                db_pool_clone,
                object_detection_model_clone,
                face_detection_model_clone,
                face_recognition_model_clone,
                face_database_clone,
            )
            .await;
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
    object_detection_model: Arc<RwLock<Model>>,
    face_detection_model_clone: Arc<RwLock<Model>>,
    face_recognition_model_clone: Arc<RwLock<Model>>,
    face_database_clone: Arc<RwLock<FaceDatabase>>,
) -> JoinHandle<()> {
    let mut consumer = Consumer::new(channel, QueueType::VideoProcessing)
        .await
        .expect("Failed to create consumer");

    let processing_job_repo = ProcessingJobsRepository::new(db.clone());
    let organization_repo = OrganizationRepository::new(db);

    consumer.set_handler(move |data, _routing| {
        let processing_job_repo_clone = processing_job_repo.clone();
        let organization_repo_clone = organization_repo.clone();
        let object_detection_model_clone = object_detection_model.clone();
        let face_detection_model_clone = face_detection_model_clone.clone();
        let face_recognition_model_clone = face_recognition_model_clone.clone();
        let face_database_clone = face_database_clone.clone();

        handle_message(
            data.to_vec(),
            processing_job_repo_clone,
            organization_repo_clone,
            object_detection_model_clone,
            face_detection_model_clone,
            face_recognition_model_clone,
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
    organization_repo: OrganizationRepository,
    object_detection_model: Arc<RwLock<Model>>,
    face_detection_model: Arc<RwLock<Model>>,
    face_recognition_model: Arc<RwLock<Model>>,
    worker_id: usize,
) -> BoxFuture<'static, std::result::Result<(), anyhow::Error>> {
    Box::pin(async move {
        let mut cancelled = false;
        let payload = Payload::deserialize(&data)?;

        let inner_res: Result<(), anyhow::Error> = (|| async {
            let video_job = processing_job_repo
                .get_video_job_by_id(&payload.video_job_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Video job not found"))?;

            if video_job.status == ProcessingStatus::Cancelled {
                cancelled = true;
                return Ok(());
            }

            let camera_presets = organization_repo
                .get_camera_preset_by_id(&video_job.camera_presets_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("CameraPreset not found"))?;

            if camera_presets.detectors.is_empty() {
                return Err(anyhow::anyhow!("Detectors not found"));
            }

            let detectors_struct = Detectors {
                face: if camera_presets.detectors.contains(&DetectorType::Face) {
                    Some(FaceModels {
                        detection: face_detection_model,
                        recognition: face_recognition_model,
                    })
                } else {
                    None
                },
                person: if camera_presets.detectors.contains(&DetectorType::Person) {
                    Some(object_detection_model)
                } else {
                    None
                },
            };

            processing_job_repo
                .update_video_jobs_status(&video_job.id, ProcessingStatus::Processing)
                .await?;

            let rtmp_url = format!("rtmp://localhost/live/stream_{}", video_job.id);
            let motion = Arc::new(Mutex::new(Motion::new()?));
            let detection_state = Arc::new(Mutex::new(DetectionState::new(
                motion,
                detectors_struct,
                24,
                300,
            )));

            let detection_state_clone = detection_state.clone();

            let mut pipeline = GstPipeline::new(
                &video_job.file_path,
                &rtmp_url,
                move |buffer, bounding_box| {
                    if let Err(e) = process_buffer(buffer, &detection_state_clone, bounding_box) {
                        eprintln!("Error processing buffer: {}", e);
                    }
                },
            )?;

            let stop_flag = pipeline.get_stop_flag();
            let cancel_rx = register_video_processing(payload.video_job_id, stop_flag.clone());

            let pipeline_handle = tokio::task::spawn_blocking(move || pipeline.run());

            tokio::select! {
                _ = cancel_rx => {
                    cancelled = true;
                    stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }
                run_res = pipeline_handle => {
                    match run_res {
                        Ok(Ok(())) => Ok(()),
                        Ok(Err(e)) => Err(anyhow!("Pipeline error: {:?}", e)),
                        Err(e) => Err(anyhow!("Join error: {:?}", e)),
                    }
                }
            }
        })()
        .await;

        let final_status = if cancelled {
            ProcessingStatus::Cancelled
        } else if inner_res.is_err() {
            ProcessingStatus::Failed
        } else {
            ProcessingStatus::Completed
        };

        processing_job_repo
            .update_video_jobs_status(&payload.video_job_id, final_status)
            .await?;

        unregister_video_processing(&payload.video_job_id);

        inner_res
    })
}

fn process_buffer(
    buffer: &VideoFrame<Readable>,
    state: &Arc<Mutex<DetectionState>>,
    bounding_box: &Arc<Mutex<Vec<(BoundingBox, usize, f32)>>>,
) -> Result<()> {
    let mut state_guard = state.lock();
    let _ = state_guard.process_frame(buffer, bounding_box)?;

    Ok(())
}
