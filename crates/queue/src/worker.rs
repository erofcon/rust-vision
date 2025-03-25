use crate::utils::{TaskType, WorkerConfig};
use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueDeclareOptions};
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties, Consumer, Queue};
use serde_json::Value;
use std::sync::Arc;

/// Task handler implemented by the library user
#[async_trait]
pub trait JobHandler: Send + Sync + 'static {
    /// Method for processing received tasks
    ///
    /// # Parameters
    /// * `payload` - Message body as a byte array
    ///
    /// # Returns
    /// * `Result<(), Box<dyn std::error::Error>>` - Processing result

    async fn handle_task(
        &self,
        job_type: TaskType,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

/// Basic worker class for processing tasks from RabbitMQ
pub struct Worker<H: JobHandler> {
    config: WorkerConfig,
    connection: Connection,
    channel: Channel,
    queue: Queue,
    handler: Arc<H>,
    consumer: Option<Consumer>,
}

impl<H: JobHandler> Worker<H> {
    /// Creates a new worker instance
    ///
    /// # Parameters
    /// * `config` - Configuration for the worker
    /// * `handler` - Task handler
    ///
    /// # Returns
    /// * `Result<Self, Box<dyn std::error::Error>>` - Result of worker creation
    pub async fn new(config: WorkerConfig, handler: H) -> Result<Self, Box<dyn std::error::Error>> {
        let connection = Connection::connect(&config.url, ConnectionProperties::default()).await?;
        let channel = connection.create_channel().await?;

        // Setting up prefetch for the channel
        channel
            .basic_qos(
                config.prefetch_count,
                lapin::options::BasicQosOptions::default(),
            )
            .await?;

        let queue = channel
            .queue_declare(
                &config.queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await?;

        Ok(Self {
            config,
            connection,
            channel,
            queue,
            handler: Arc::new(handler),
            consumer: None,
        })
    }

    /// Starts the worker and begins processing messages
    ///
    /// # Returns
    /// * `Result<(), Box<dyn std::error::Error>>` - The result of starting the worker
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting a worker for a queue: {}", self.config.queue_name);

        let mut consumer = self
            .channel
            .basic_consume(
                &self.config.queue_name,
                &self.config.consumer_tag,
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        let channel = self.channel.clone();
        let handler = self.handler.clone();

        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let delivery_tag = delivery.delivery_tag;
                    let payload = delivery.data.to_vec();
                    let channel_clone = channel.clone();
                    let handler_clone = handler.clone();

                    let job_type = match Self::parse_job_type(&payload) {
                        Ok(job_type) => job_type,
                        Err(e) => {
                            eprintln!("Invalid job type: {}", e);
                            if let Err(e) = channel_clone
                                .basic_nack(delivery_tag, BasicNackOptions::default())
                                .await
                            {
                                eprintln!("Error rejecting message: {}", e);
                            }
                            continue;
                        }
                    };

                    tokio::spawn(async move {
                        match handler_clone.handle_task(job_type, &payload).await {
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
        println!("Stopping a worker for a queue: {}", self.config.queue_name);
        self.channel.close(0, "Normal completion").await?;
        self.connection.close(0, "Normal completion").await?;
        Ok(())
    }

    pub fn parse_job_type(payload: &[u8]) -> Result<TaskType, Box<dyn std::error::Error>> {
        let data: Value = serde_json::from_slice(payload)?;

        match data["task_type"].as_str() {
            Some("run_pipeline") => Ok(TaskType::RunPipeline),
            Some("generate_report") => Ok(TaskType::GenerateReport),
            _ => Err("Unknown job type".into()),
        }
    }
}
