// TODO: to delete

use crate::utils::QueueType;
use anyhow::{Context, Result};
use lapin::options::QueueDeclareOptions;
use lapin::types::FieldTable;
use lapin::{Channel, Connection, ConnectionProperties};

pub struct Publisher {
    channel: Channel,
}

impl Publisher {
    pub async fn new(url: &str) -> Result<Self> {
        let connection = Connection::connect(url, ConnectionProperties::default()).await?;

        let channel = connection.create_channel().await?;

        Ok(Self { channel })
    }

    pub async fn publish(&self, payload: &[u8], queue_type: QueueType) -> Result<()> {
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
            .await
            .context("Error creating queue")?;

        Ok(())
    }

    // TODO: add ping

    pub async fn close(&self) -> Result<()> {
        self.channel.close(200, "Bye").await?;
        Ok(())
    }
}
