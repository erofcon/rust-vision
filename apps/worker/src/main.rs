use async_trait::async_trait;
use queue::utils::QueueType;
use queue::worker::{QueueHandler, Worker};
use std::error::Error;

struct TaskHandler;

#[async_trait]
impl QueueHandler for TaskHandler {
    async fn queue_handler(
        &self,
        queue_type: &QueueType,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        println!("Task completed: {}", queue_type.to_str());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting RabbitMQ worker ...");

    let handler = TaskHandler;

    let mut worker = Worker::new(
        "amqp://guest:guest@localhost:5672".into(),
        QueueType::RunPipeline,
        handler,
    )
    .await?;

    worker.start().await?;

    Ok(())
}
