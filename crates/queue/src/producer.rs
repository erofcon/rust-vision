use crate::utils::QueueType;
use anyhow::{Context, Result};
use lapin::options::QueueDeclareOptions;
use lapin::types::FieldTable;
use lapin::Channel;

pub struct Producer {
    channel: Channel,
}

impl Producer {
    pub fn new(channel: Channel) -> Result<Self> {
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
            .await
            .context("Error declaring queue")?;

        self.channel
            .basic_publish(
                "",
                &queue_type.to_str(),
                lapin::options::BasicPublishOptions::default(),
                payload,
                lapin::BasicProperties::default().with_delivery_mode(2),
            )
            .await
            .context("Error when publishing queue")?;

        Ok(())
    }
}
