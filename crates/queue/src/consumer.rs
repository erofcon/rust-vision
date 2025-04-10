use crate::utils::QueueType;
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, QueueDeclareOptions};
use lapin::types::{FieldTable, ShortString};
use lapin::Channel;
use std::sync::Arc;

#[async_trait]
pub trait QueueHandler: Send + Sync + 'static {
    async fn handler(&self, payload: &[u8], routing_key: &ShortString) -> Result<()>;
}

pub struct Consumer<H: QueueHandler> {
    channel: Channel,
    consumer: lapin::Consumer,
    handler: Arc<H>,
}

impl<H: QueueHandler> Consumer<H> {
    pub async fn new(channel: Channel, queue_type: QueueType, handler: H) -> Result<Self> {
        channel
            .queue_declare(
                &queue_type.to_str(),
                QueueDeclareOptions {
                    durable: true,
                    ..QueueDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await
            .context("Failed to declare queue in consumer")?;

        let consumer = channel
            .basic_consume(
                &queue_type.to_str(),
                "consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        Ok(Self {
            channel,
            consumer,
            handler: Arc::new(handler),
        })
    }

    pub async fn start(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let handler = self.handler.clone();
        let channel = self.channel.clone();

        while let Some(delivery) = self.consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let delivery_tag = delivery.delivery_tag;
                    let handler_clone = handler.clone();
                    let channel_clone = channel.clone();

                    tokio::spawn(async move {
                        match handler_clone
                            .handler(&delivery.data, &delivery.routing_key)
                            .await
                        {
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
}
