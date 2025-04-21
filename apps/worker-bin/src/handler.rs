use crate::frame_processor::build_frame_processor;
use anyhow::Result;
use common::config::ModelConfig;
use common::pipeline_registry::ACTIVE_PIPELINES;
use futures::future::BoxFuture;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use queue::consumer::Consumer;
use queue::utils::Payload;
use queue::utils::QueueType;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use storage::models::video::VideoStatus;
use storage::repositories::video_repository::VideoRepository;
use streaming::file_pipeline::FilePipeline;
use tokio::task::JoinHandle;

pub async fn spawn_worker(
    worker_id: usize,
    channel: lapin::Channel,
    video_repo: Arc<VideoRepository>,
    model_cfg: Arc<ModelConfig>,
) -> JoinHandle<()> {
    let mut consumer = Consumer::new(channel, QueueType::VideoProcessing)
        .await
        .expect("Failed to create consumer");

    consumer.set_handler(move |data, routing| {
        handle_message(
            worker_id,
            data.to_vec(),
            routing.to_string(),
            Arc::clone(&video_repo),
            Arc::clone(&model_cfg),
        )
    });

    tokio::spawn(async move {
        if let Err(e) = consumer.start().await {
            eprintln!("[Worker {}] Consumer error: {}", worker_id, e);
        }
    })
}

fn handle_message(
    worker_id: usize,
    data: Vec<u8>,
    routing: String,
    video_repo: Arc<VideoRepository>,
    model_cfg: Arc<ModelConfig>,
) -> BoxFuture<'static, Result<(), anyhow::Error>> {
    Box::pin(async move {
        let payload = Payload::deserialize(&data)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize: {}", e))?;

        println!("[Worker {}] Routing Key: {}", worker_id, routing);
        println!("[Worker {}] Payload ID: {}", worker_id, payload.id);

        let video_result = video_repo.get_by_id(payload.id).await?;

        let video = match video_result {
            Some(video) => video,
            None => {
                return Err(anyhow::anyhow!("Error to get video"));
            }
        };

        if video.status == VideoStatus::Cancelled {
            println!("[Worker {}] Video is cancelled", worker_id);
            return Ok(());
        }

        video_repo
            .change_status(payload.id, &VideoStatus::Processing)
            .await?;

        let stream_url = format!("rtmp://localhost/live/stream_{}", video.id);

        let mut pipeline = FilePipeline::new(
            &video.file_path,
            model_cfg.input_width,
            model_cfg.input_height,
            &stream_url,
        )?;

        let processor_arc = build_frame_processor();
        let processor = move |frame: &VideoFrame<Readable>| processor_arc(frame);
        pipeline.set_frame_processor(processor);

        let video_id = video.id;
        let cancel_flag = pipeline.get_cancel_flag();
        ACTIVE_PIPELINES.write().await.insert(video_id, cancel_flag);

        let pipeline_task = tokio::task::spawn_blocking(move || {
            let result = pipeline.start();
            if let Err(e) = &result {
                eprintln!("[Worker {}] Pipeline error: {}", worker_id, e);
            }
            result
        });

        let video_id_clone = video_id;
        let video_repo_clone = Arc::clone(&video_repo);

        // TODO: find a solution that doesn't poll the database as often
        let cancel_checker = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;

                match video_repo_clone.get_by_id(video_id_clone).await {
                    Ok(Some(video)) if video.status == VideoStatus::Cancelled => {
                        if let Some(flag) = ACTIVE_PIPELINES.read().await.get(&video_id_clone) {
                            flag.store(true, Ordering::SeqCst);
                        }
                        break;
                    }
                    Err(e) => {
                        eprintln!("[Worker {}] Error checking video status: {}", worker_id, e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        pipeline_task.await?.expect("Failed to spawn pipeline");
        cancel_checker.abort();

        ACTIVE_PIPELINES.write().await.remove(&video_id);

        match video_repo.get_by_id(video_id).await? {
            Some(video) if video.status != VideoStatus::Cancelled => {
                video_repo
                    .change_status(video_id, &VideoStatus::Completed)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    })
}
