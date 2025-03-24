use lapin::{Channel, Connection, ConnectionProperties};
use lapin::options::QueueDeclareOptions;
use lapin::types::FieldTable;

pub struct Publisher {
    channel: Channel,
    queue_name: String,
}


impl Publisher {
    /// Creates a new publisher instance
    ///
    /// # Parameters
    /// * `url` - URL connection to RabbitMQ
    /// * `queue_name` - Name of the queue to publish
    ///
    /// # Returns
    /// * `Result<Self, Box<dyn std::error::Error>>` - Result of creating the publisher
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

    /// Publishes a message to the queue
    ///
    /// # Parameters
    /// * `payload` - Message body
    ///
    /// # Returns
    /// * `Result<(), Box<dyn std::error::Error>>` - Publish result
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