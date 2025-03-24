use queue::publisher::Publisher;
use std::env;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let message = env::args()
        .nth(1)
        .unwrap_or_else(|| "Тестовое сообщение".to_string());
    println!("Публикация сообщения: {}", message);

    let publisher = Publisher::new("amqp://guest:guest@localhost:5672", "tasks").await?;

    publisher.publish(message.as_bytes()).await?;

    println!("Сообщение успешно опубликовано!");

    Ok(())
}
