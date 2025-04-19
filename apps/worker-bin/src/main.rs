use anyhow::Result;
use common::config::{CommonConfig, ModelConfig, WorkerConfig};
use futures::future::join_all;
use queue::connection::MQ;
use queue::consumer::Consumer;
use queue::utils::{Payload, QueueType};
use std::error::Error;
use std::sync::Arc;
use storage::database::Database;
use storage::repositories::video_repository::VideoRepository;
use streaming::file_pipeline::FilePipeline;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    unsafe {
        std::env::set_var("GST_DEBUG", "3");
        std::env::set_var("RUST_BACKTRACE", "full");
    }

    gst::init()?;

    println!("Starting RabbitMQ workers ...");
    let common_config = CommonConfig::load()?;
    let worker_config = WorkerConfig::load()?;
    let model_config = ModelConfig::load()?;

    let db = Database::new(&common_config.database.connection_string()).await?;
    let video_repo = Arc::new(VideoRepository::new(db.get_pool().clone()));

    let mq = MQ::new(&common_config.mq.connection_to_string()).await?;

    let mut handles = Vec::with_capacity(worker_config.worker_count);

    for worker_id in 0..worker_config.worker_count {
        let channel = mq.get_channel().clone();
        let video_repo = video_repo.clone();
        let model_config = Arc::new(model_config.clone());

        let mut consumer = Consumer::new(channel, QueueType::VideoProcessing).await?;

        consumer.set_handler(move |payload_bytes, routing_key| {
            let routing = routing_key.as_str().to_string();
            let data = payload_bytes.to_vec();
            let video_repo = video_repo.clone();
            let model_config = model_config.clone();

            async move {
                let payload = Payload::deserialize(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to deserialize payload: {}", e))?;

                println!("[Worker {}] Routing Key: {}", worker_id, routing);
                println!(
                    "[Worker {}] Start handling Payload ID: {}",
                    worker_id, payload.id
                );

                let video = video_repo.get_by_id(payload.id).await?.unwrap();

                let stream_url = format!("rtmp://localhost/live/stream_{}", Uuid::new_v4());

                let mut pipeline = FilePipeline::new(
                    &video.file_path,
                    model_config.input_width,
                    model_config.input_height,
                    &stream_url,
                )?;

                pipeline.set_frame_processor(move |_frame| Ok(()));

                pipeline.start().expect("Failed to start FilePipeline");

                // TODO: дальнейшая логика обработки
                Ok(())
            }
        });

        let handle = tokio::spawn(async move {
            if let Err(e) = consumer.start().await {
                eprintln!("[Worker {}] Consumer error: {}", worker_id, e);
            }
        });

        handles.push(handle);
    }

    join_all(handles).await;

    Ok(())
}
