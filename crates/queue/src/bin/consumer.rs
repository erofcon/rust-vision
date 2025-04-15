use anyhow::Result;
use async_trait::async_trait;
use lapin::types::ShortString;
use queue::connection::MQ;
use queue::consumer::{Consumer, QueueHandler};
use queue::utils::{Payload, QueueType};
use std::error::Error;

struct TaskHandler;

#[async_trait]
impl QueueHandler for TaskHandler {
    async fn handler(&self, payload: &[u8], routing_key: &ShortString) -> Result<()> {
        let payload = Payload::deserialize(payload)?;

        println!("Task completed: {}", payload.id);
        println!("Task completed: {}", routing_key.as_str());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting RabbitMQ worker ...");

    let handler = TaskHandler;

    let mq = MQ::new("amqp://guest:guest@localhost:5672").await?;

    let channel = mq.get_channel().clone();

    let mut worker = Consumer::new(channel, QueueType::VideoProcessing, handler).await?;

    worker.start().await?;

    Ok(())
}
