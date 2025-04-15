// TODO: delete in production. make separate crate
use anyhow::Result;
use common::config::CommonConfig;
use queue::connection::MQ;
use queue::consumer::Consumer;
use queue::utils::{Payload, QueueType};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting RabbitMQ worker ...");
    let common_config = CommonConfig::load()?;

    let mq = MQ::new(&common_config.mq.connection_string()).await?;

    let channel = mq.get_channel().clone();

    let mut consumer = Consumer::new(channel, QueueType::VideoProcessing).await?;

    consumer.set_handler(move |payload, routing_key| {
        let payload_vec = payload.to_vec();
        let routing_key_str = routing_key.as_str().to_string();

        async move {
            let payload = Payload::deserialize(&payload_vec).unwrap();

            println!("{}", routing_key_str);
            println!("{}", payload.id);

            Ok(())
        }
    });

    consumer.start().await?;

    Ok(())
}
