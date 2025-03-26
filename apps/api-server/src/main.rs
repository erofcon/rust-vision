use queue::publish::Publisher;
use queue::utils::QueueType;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let message = r#"{"message": "Hello world", "task_type": "generate_report"}"#;

    println!("Публикация сообщения: {}", message);

    let publisher = Publisher::new("amqp://guest:guest@localhost:5672").await?;

    publisher
        .publish(message.as_bytes(), QueueType::GenerateReport)
        .await?;

    println!("Сообщение успешно опубликовано!");

    Ok(())
}
