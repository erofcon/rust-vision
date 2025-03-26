use crate::utils::QueueType;
use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueDeclareOptions};
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties};
use std::sync::Arc;

/// Queue handler implemented by the library user
#[async_trait]
pub trait QueueHandler: Send + Sync + 'static {
    /// Method for processing received tasks
    ///
    /// # Parameters
    /// * `queue_type` - Queue type
    ///
    /// # Returns
    /// * `Result<(), Box<dyn std::error::Error>>` - Processing result
    async fn queue_handler(
        &self,
        queue_type: &QueueType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Basic worker class for processing tasks from RabbitMQ
pub struct Worker<H: QueueHandler> {
    connection: Connection,
    channel: Channel,
    queue_type: QueueType,
    handler: Arc<H>,
}

impl<H: QueueHandler> Worker<H> {
    /// Creates a new worker instance
    ///
    /// # Parameters
    /// * `url`
    /// * `queue_type`
    /// * `handler` - Task handler
    ///
    /// # Returns
    /// * `Result<Self, Box<dyn std::error::Error>>` - Result of worker creation

    pub async fn new(
        url: &str,
        queue_type: QueueType,
        handler: H,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = Connection::connect(&url, ConnectionProperties::default()).await?;
        let channel = connection.create_channel().await?;

        channel
            .queue_declare(
                &queue_type.to_str(),
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;
        Ok(Self {
            connection,
            channel,
            queue_type,
            handler: Arc::new(handler),
        })
    }

    /// Starts the worker and begins processing messages
    ///
    /// # Returns
    /// * `Result<(), Box<dyn std::error::Error>>` - The result of starting the worker
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut consumer = self
            .channel
            .basic_consume(
                &self.queue_type.to_str(),
                "consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let handler = self.handler.clone();
        let channel = self.channel.clone();
        let queue_type = self.queue_type.clone();

        println!(
            "Starting a worker for a queue: {} \n [*] Waiting for messages. To exit press CTRL+C",
            queue_type.to_str()
        );

        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let delivery_tag = delivery.delivery_tag;
                    let handler_clone = handler.clone();
                    let channel_clone = channel.clone();
                    let queue_type_clone = queue_type.clone();

                    tokio::spawn(async move {
                        match handler_clone.queue_handler(&queue_type_clone).await {
                            Ok(_) => {
                                if let Err(e) = channel_clone
                                    .basic_ack(delivery_tag, BasicAckOptions::default())
                                    .await
                                {
                                    eprintln!("Error confirming message: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("Error processing message: {}", e);
                                if let Err(e) = channel_clone
                                    .basic_nack(delivery_tag, BasicNackOptions::default())
                                    .await
                                {
                                    eprintln!("Error rejecting message: {}", e);
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Error receiving message: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Stops the worker
    ///
    /// # Returns
    /// * `Result<(), Box<dyn std::error::Error>>` - The result of stopping the worker
    pub async fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!(
            "Stopping a worker for a queue: {}",
            self.queue_type.to_str()
        );
        self.channel.close(0, "Normal completion").await?;
        self.connection.close(0, "Normal completion").await?;
        Ok(())
    }
}
