use anyhow::{Context, Result};
use lapin::{Channel, Connection, ConnectionProperties};

pub struct MQ {
    channel: Channel,
}

impl MQ {
    pub async fn new(url: &str) -> Result<Self> {
        let connection = Connection::connect(url, ConnectionProperties::default())
            .await
            .context("Connection is failure")?;

        let channel = connection
            .create_channel()
            .await
            .context("Failed to create channel")?;

        Ok(Self { channel })
    }

    pub fn get_channel(&self) -> &Channel {
        &self.channel
    }

    pub async fn close(&self) -> Result<()> {
        self.channel
            .close(200, "OK")
            .await
            .context("Error closing channel")?;

        Ok(())
    }

    // TODO: add ping
}
