use crate::utils::QueueType;
use lapin::options::QueueDeclareOptions;
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties};

pub struct Publisher {
    channel: Channel,
}

impl Publisher {
    /// Creates a new publisher instance
    ///
    /// # Parameters
    /// * `url` - URL connection to RabbitMQ
    ///
    /// # Returns
    /// * `Result<Self, Box<dyn std::error::Error>>` - Result of creating the publisher
    pub async fn new(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = Connection::connect(url, ConnectionProperties::default()).await?;

        let channel = connection.create_channel().await?;

        Ok(Self { channel })
    }

    /// Publishes a message to the queue
    ///
    /// # Parameters
    /// * `payload` - Message body
    /// * `queue_type` - Type of the queue to publish
    ///
    /// # Returns
    /// * `Result<(), Box<dyn std::error::Error>>` - Publish result
    pub async fn publish(
        &self,
        payload: &[u8],
        queue_type: QueueType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.channel
            .queue_declare(
                queue_type.to_str(),
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;

        self.channel
            .basic_publish(
                "",
                &queue_type.to_str(),
                lapin::options::BasicPublishOptions::default(),
                payload,
                lapin::BasicProperties::default().with_delivery_mode(2),
            )
            .await?;

        Ok(())
    }
}
