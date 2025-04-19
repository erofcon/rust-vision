use crate::frame_processor::build_frame_processor;
use anyhow::Result;
use common::config::ModelConfig;
use futures::future::BoxFuture;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
use queue::consumer::Consumer;
use queue::utils::Payload;
use queue::utils::QueueType;
use std::sync::Arc;
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
        // async_std::task::sleep(Duration::from_secs(5)).await;

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

        pipeline.start()?;

        video_repo
            .change_status(payload.id, &VideoStatus::Completed)
            .await?;

        Ok(())
    })
}
