use anyhow::{Result, anyhow};
use common::pipeline_registry::{register_video_processing, unregister_video_processing};
use futures::future::BoxFuture;
use gst_streaming::pipeline::GstPipeline;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use inference_core::detection_state::DetectionState;
use inference_core::detectors::{Detectors, FaceDetectors};
use inference_core::globals::{
    FACE_DETECTION_MODEL, FACE_RECOGNITION_MODEL, OBJECT_DETECTION_MODEL,
};
use inference_core::utils::BoundingBox;
use motion::motion::Motion;
use parking_lot::Mutex;
use queue::consumer::Consumer;
use queue::global::get_mq;
use queue::utils::{Payload, QueueType};
use std::sync::Arc;
use storage::global::get_database;
use storage::models::organization::DetectorType;
use storage::models::processing_job::ProcessingStatus;
use storage::repositories::organization_repository::OrganizationRepository;
use storage::repositories::processing_jobs_repository::ProcessingJobsRepository;
use tokio::task::JoinHandle;

pub async fn spawn_worker(worker_id: usize) -> JoinHandle<()> {
    let database = get_database().await.get_pool().clone();
    let mq = get_mq().await.get_channel().clone();

    let mut consumer = Consumer::new(mq, QueueType::VideoProcessing)
        .await
        .expect("Failed to create consumer");

    let processing_job_repo = ProcessingJobsRepository::new(database.clone());
    let organization_repo = OrganizationRepository::new(database);

    consumer.set_handler(move |data, _routing| {
        let processing_job_repo_clone = processing_job_repo.clone();
        let organization_repo_clone = organization_repo.clone();

        handle_message(
            data.to_vec(),
            processing_job_repo_clone,
            organization_repo_clone,
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
    worker_id: usize,
) -> BoxFuture<'static, std::result::Result<(), anyhow::Error>> {
    Box::pin(async move {
        let mut cancelled = false;
        let payload = Payload::deserialize(&data)?;

        // Убираем лишнее замыкание, выполняем логику напрямую
        let inner_res: Result<(), anyhow::Error> = async {
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
                    Some(FaceDetectors {
                        detection: FACE_DETECTION_MODEL.clone(),
                        recognition: FACE_RECOGNITION_MODEL.clone(),
                    })
                } else {
                    None
                },
                person: if camera_presets.detectors.contains(&DetectorType::Person) {
                    Some(OBJECT_DETECTION_MODEL.clone())
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
        }
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
