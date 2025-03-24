use async_trait::async_trait;
use queue::utils::WorkerConfig;
use queue::worker::{JobHandler, Worker};
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;

struct TaskHandler;

#[async_trait]
impl JobHandler for TaskHandler {
    async fn handle_run_pipeline(
        &self,
        payload: &[u8],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let message = String::from_utf8_lossy(payload);
        println!("Task handler: {}", message);

        sleep(Duration::from_secs(1)).await;

        println!("Task '{}' completed", message);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Запуск обработчика задач RabbitMQ");

    let config = WorkerConfig {
        url: "amqp://guest:guest@localhost:5672".into(),
        queue_name: "tasks".into(),
        prefetch_count: 10,
        consumer_tag: "task_worker".into(),
    };

    let handler = TaskHandler;

    let mut worker = Worker::new(config,  handler).await?;

    println!("Воркер запущен и ожидает задачи...");

    // Запускаем обработку задач
    worker.start().await?;

    Ok(())
}
