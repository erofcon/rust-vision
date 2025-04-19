use crate::utils::QueueType;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions, QueueDeclareOptions,
};
use lapin::types::{FieldTable, ShortString};
use lapin::Channel;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

type HandlerFn = Arc<
    dyn Fn(&[u8], &ShortString) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync,
>;

pub struct Consumer {
    channel: Channel,
    consumer: lapin::Consumer,
    handler: Option<HandlerFn>,
}

impl Consumer {
    pub async fn new(
        channel: Channel,
        queue_type: QueueType,
    ) -> Result<Self> {
        channel.basic_qos(1, BasicQosOptions::default()).await?;

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
            handler: None,
        })
    }

    pub fn set_handler<F, Fut>(&mut self, handler_fn: F)
    where
        F: Fn(&[u8], &ShortString) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let handler: HandlerFn =
            Arc::new(move |payload, routing_key| Box::pin(handler_fn(payload, routing_key)));

        self.handler = Some(handler);
    }

    pub async fn start(&mut self) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let handler = match self.handler.clone() {
            Some(handler) => handler,
            None => return Err("No handler set for consumer".into()),
        };

        let channel = self.channel.clone();

        while let Some(delivery) = self.consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let delivery_tag = delivery.delivery_tag;
                    let handler_clone = handler.clone();
                    let channel_clone = channel.clone();

                    tokio::spawn(async move {
                        // TODO: add logging
                        match handler_clone(&delivery.data, &delivery.routing_key).await {
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
