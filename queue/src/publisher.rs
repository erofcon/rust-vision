use lapin::{Channel, Connection, ConnectionProperties};
use lapin::options::QueueDeclareOptions;
use lapin::types::FieldTable;

pub struct Publisher {
    channel: Channel,
    queue_name: String,
}


impl Publisher {
    /// Создает новый экземпляр публикатора
    ///
    /// # Параметры
    /// * `url` - URL подключения к RabbitMQ
    /// * `queue_name` - Имя очереди для публикации
    ///
    /// # Возвращает
    /// * `Result<Self, Box<dyn std::error::Error>>` - Результат создания публикатора
    pub async fn new(url: &str, queue_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = Connection::connect(
            url,
            ConnectionProperties::default(),
        ).await?;

        let channel = connection.create_channel().await?;

        channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;

        Ok(Self {
            channel,
            queue_name: queue_name.to_string(),
        })
    }

    /// Публикует сообщение в очередь
    ///
    /// # Параметры
    /// * `payload` - Тело сообщения
    ///
    /// # Возвращает
    /// * `Result<(), Box<dyn std::error::Error>>` - Результат публикации
    pub async fn publish(&self, payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.channel
            .basic_publish(
                "",
                &self.queue_name,
                lapin::options::BasicPublishOptions::default(),
                payload,
                lapin::BasicProperties::default()
                    .with_delivery_mode(2),
            )
            .await?;

        Ok(())
    }
}