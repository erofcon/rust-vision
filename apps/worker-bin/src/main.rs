use anyhow::Result;
use common::config::{CommonConfig, WorkerConfig};
use queue::connection::MQ;
use queue::consumer::Consumer;
use queue::utils::{Payload, QueueType};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting RabbitMQ workers ...");
    let common_config = CommonConfig::load()?;
    let worker_config = WorkerConfig::load()?;

    let mq = MQ::new(&common_config.mq.connection_to_string()).await?;

    let mut handles = Vec::with_capacity(worker_config.worker_count);

    for worker_id in 0..worker_config.worker_count {
        let channel = mq.get_channel().clone();

        let mut consumer = Consumer::new(channel, QueueType::VideoProcessing).await?;

        consumer.set_handler(move |payload_bytes, routing_key| {
            let routing = routing_key.as_str().to_string();
            let data = payload_bytes.to_vec();

            async move {
                let payload = Payload::deserialize(&data)
                    .map_err(|e| anyhow::anyhow!("Failed to deserialize payload: {}", e))?;

                async_std::task::sleep(std::time::Duration::from_secs(5)).await;

                println!("[Worker {}] Routing Key: {}", worker_id, routing);
                println!("[Worker {}] Payload ID: {}", worker_id, payload.id);

                // TODO: actual processing logic here

                Ok(())
            }
        });

        // Spawn each consumer in its own task
        let handle = tokio::spawn(async move {
            if let Err(e) = consumer.start().await {
                eprintln!("[Worker {}] Consumer error: {}", worker_id, e);
            }
        });

        handles.push(handle);
    }

    // Wait for all worker tasks to complete (runs indefinitely)
    futures::future::join_all(handles).await;

    Ok(())
}
