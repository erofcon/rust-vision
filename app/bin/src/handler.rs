use anyhow::{Result, anyhow};
use common::config::BaseDetectionModel;
use common::pipeline_registry::{register_video_processing, unregister_video_processing};
use futures::future::BoxFuture;
use gst_video::VideoFrame;
use gst_video::video_frame::Readable;
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
    let mut consumer = Consumer::new(channel, QueueType::VideoProcessing)
        .await
        .expect("Failed to create consumer");

    // Set message handler
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

    // Start consuming
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

            let stream_url = format!("rtmp://localhost/live/stream{}", id);
            let mut pipeline =
                FilePipeline::new(&video.file_path, input_width, input_height, &stream_url)?;
            let stop_flag = pipeline.get_stop_flag();
            let cancel_rx = register_video_processing(id, stop_flag.clone());

            pipeline.set_frame_processor(move |frame, original_w, original_h| {
                frame_processor(frame, original_w, original_h)
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

        // let payload = Payload::deserialize(&data)?;
        //
        // println!(
        //     "[Worker {}] Routing: {}, ID: {}",
        //     worker_id, routing, payload.id
        // );
        //
        // // Fetch video record
        // let video = video_repo
        //     .get_by_id(payload.id)
        //     .await?
        //     .ok_or_else(|| anyhow::anyhow!("Video not found"))?;
        //
        // if video.status == VideoStatus::Cancelled {
        //     println!("[Worker {}] Video cancelled", worker_id);
        //     return Ok(());
        // }
        //
        // // Update status to Processing
        // video_repo
        //     .change_status(payload.id, &VideoStatus::Processing)
        //     .await
        //     .ok();
        //
        // // Prepare pipeline
        // let stream_url = format!("rtmp://localhost/live/stream{}", video.id);
        // let mut pipeline =
        //     FilePipeline::new(&video.file_path, input_width, input_height, &stream_url)?;
        //
        // let stop_flag = pipeline.get_stop_flag();
        // let cancel_rx = register_video_processing(video.id, stop_flag.clone());
        //
        // // Attach frame processor directly
        // pipeline.set_frame_processor(move |frame| frame_processor(frame));
        //
        // // Run pipeline in blocking task
        // let pipeline_handle = tokio::task::spawn_blocking(move || pipeline.start());
        //
        // tokio::select! {
        //     _ = cancel_rx => {
        //         println!("[Worker {}] Cancel signal received", worker_id);
        //         stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        //     }
        //     result = pipeline_handle => {
        //         if let Err(e) = result {
        //             eprintln!("[Worker {}] Pipeline error: {:?}", worker_id, e);
        //         }
        //     }
        // }
        //
        // // Finalize status
        // let final_status = if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
        //     VideoStatus::Cancelled
        // } else {
        //     VideoStatus::Completed
        // };
        // video_repo
        //     .change_status(payload.id, &final_status)
        //     .await
        //     .ok();
        // unregister_video_processing(&video.id);
        //
        // Ok(())
    })
}
