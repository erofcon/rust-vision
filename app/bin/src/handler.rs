use anyhow::{Result, anyhow};
use common::pipeline_registry::{register_video_processing, unregister_video_processing};
use futures::future::BoxFuture;
use queue::consumer::Consumer;
use queue::utils::{Payload, QueueType};
use std::sync::Arc;
use storage::models::video::VideoStatus;
use storage::repositories::video_repository::VideoRepository;
use streaming::file_pipeline::FilePipeline;
use streaming::utils::FrameProcessor;
use tokio::task::JoinHandle;

pub async fn spawn_worker(
    worker_id: usize,
    channel: lapin::Channel,
    video_repo: Arc<VideoRepository>,
    frame_processor: Arc<FrameProcessor>,
    input_width: i32,
    input_height: i32,
) -> JoinHandle<()> {
    //TODO: check queue type

    let mut consumer = Consumer::new(channel, QueueType::VideoProcessing)
        .await
        .expect("Failed to create consumer");

    consumer.set_handler(move |data, routing| {
        handle_message(
            worker_id,
            data.to_vec(),
            routing.to_string(),
            Arc::clone(&video_repo),
            Arc::clone(&frame_processor),
            input_width,
            input_height,
        )
    });

    tokio::spawn(async move {
        if let Err(e) = consumer.start().await {
            eprintln!("[Worker {}] Consumer error: {}", worker_id, e);
        }
    })
}

fn handle_message(
    _worker_id: usize,
    data: Vec<u8>,
    _routing: String,
    video_repo: Arc<VideoRepository>,
    frame_processor: Arc<FrameProcessor>,
    input_width: i32,
    input_height: i32,
) -> BoxFuture<'static, Result<(), anyhow::Error>> {
    Box::pin(async move {
        let mut cancelled = false;
        let payload = Payload::deserialize(&data)?;
        let id = payload.id;

        video_repo
            .change_status(payload.id, &VideoStatus::Processing)
            .await
            .ok();

        let inner_res: Result<(), anyhow::Error> = (|| async {
            let video = video_repo
                .get_by_id(id)
                .await?
                .ok_or_else(|| anyhow!("Video not found: {}", id))?;

            if video.status == VideoStatus::Cancelled {
                cancelled = true;
                return Ok(());
            }

            let stream_url = format!("rtmp://localhost/live/stream_{}", id);
            let mut pipeline =
                FilePipeline::new(&video.file_path, input_width, input_height, &stream_url)?;

            let stop_flag = pipeline.get_stop_flag();
            let cancel_rx = register_video_processing(id, stop_flag.clone());

            pipeline.set_frame_processor(move |frame, bounding_box, original_w, original_h| {
                frame_processor(frame, bounding_box, original_w, original_h)
            });
            let pipeline_handle = tokio::task::spawn_blocking(move || pipeline.start());

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
            VideoStatus::Cancelled
        } else if inner_res.is_err() {
            VideoStatus::Failed
        } else {
            VideoStatus::Completed
        };

        let _ = video_repo.change_status(id, &final_status).await;
        unregister_video_processing(&id);

        inner_res
    })
}
