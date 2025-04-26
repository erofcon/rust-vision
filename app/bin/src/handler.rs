use crate::frame_processor::build_frame_processor;
use anyhow::Result;
use common::config::BaseDetectionModel;
use common::pipeline_registry::{register_video_processing, unregister_video_processing};
use futures::future::BoxFuture;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use queue::consumer::Consumer;
use queue::utils::Payload;
use queue::utils::QueueType;
use std::sync::Arc;
use storage::models::video::VideoStatus;
use storage::repositories::video_repository::VideoRepository;
use streaming::file_pipeline::FilePipeline;
use tokio::task::JoinHandle;

pub async fn spawn_worker(
    worker_id: usize,
    channel: lapin::Channel,
    video_repo: Arc<VideoRepository>,
    model_cfg: Arc<BaseDetectionModel>,
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
    model_cfg: Arc<BaseDetectionModel>,
) -> BoxFuture<'static, Result<(), anyhow::Error>> {
    // TODO: check fault tolerance

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

        match video_repo
            .change_status(payload.id, &VideoStatus::Processing)
            .await
        {
            Ok(_) => (),
            Err(e) => {
                println!("[Worker {}] Failed to change status: {}", worker_id, e);
            }
        }

        let stream_url = format!("rtmp://localhost/live/stream{}", video.id);

        let mut pipeline = match FilePipeline::new(
            &video.file_path,
            model_cfg.input_width,
            model_cfg.input_height,
            &stream_url,
        ) {
            Ok(p) => p,
            Err(e) => {
                println!("[Worker {}] Failed to create pipeline: {}", worker_id, e);
                let _ = video_repo
                    .change_status(payload.id, &VideoStatus::Failed)
                    .await;
                return Err(e);
            }
        };

        let stop_flag = pipeline.get_stop_flag();
        let cancel_rx = register_video_processing(video.id, stop_flag.clone());

        let processor_arc = build_frame_processor();
        let processor = move |frame: &VideoFrame<Readable>| processor_arc(frame);
        pipeline.set_frame_processor(processor);

        let video_id = video.id;
        let pipeline_handle = tokio::task::spawn_blocking(move || pipeline.start());
        let mut pipeline_handle = Some(pipeline_handle);

        tokio::select! {
            _ = cancel_rx => {
                println!("[Worker {}] Received cancel signal", worker_id);
                if let Some(handle) = pipeline_handle.take() {
                    stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    handle.abort();
                }
                video_repo
                    .change_status(payload.id, &VideoStatus::Cancelled)
                    .await?;
                return Ok(());
            }
            res = async {
                let handle = pipeline_handle.take()
                    .expect("Pipeline_handle should be available here");
                match handle.await {
                    Ok(inner_res) => inner_res,
                    Err(join_err) => Err(anyhow::anyhow!("Join error: {:?}", join_err)),
                }
            } => {
                match res {
                    Ok(_) => {
                        let status = if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                            VideoStatus::Cancelled
                        } else {
                            VideoStatus::Completed
                        };
                        video_repo.change_status(payload.id, &status).await?;
                    }
                    Err(_) => {
                        video_repo.change_status(payload.id, &VideoStatus::Failed).await?;
                    }
                }
            }
        }
        unregister_video_processing(&video_id);
        if let Err(e) = video_repo
            .change_status(payload.id, &VideoStatus::Completed)
            .await
        {
            println!(
                "[Worker {}] Failed to update final status: {}",
                worker_id, e
            );
        }
        Ok(())
    })
}
