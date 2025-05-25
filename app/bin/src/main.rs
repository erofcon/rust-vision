use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use anyhow::Result;
use api::handlers::{day_map, health, organization, processing_job, publish};
use common::config::ProjectConfig;
use futures::future::BoxFuture;
use queue::connection::MQ;
use queue::consumer::Consumer;
use queue::utils::{Payload, QueueType};
use storage::database::Database;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

#[actix_web::main]
async fn main() -> Result<()> {
    let config = ProjectConfig::load()?;

    // DB
    let db = Database::new(&config.database.connection_string()).await?;
    db.ping().await?;

    let db_pool = db.get_pool().clone();

    // MQ
    let mq = MQ::new(&config.mq.connection_to_string()).await?;
    let mq_channel = mq.get_channel().clone();

    // Workers
    println!("Spawning {} worker(s)...", config.worker.worker_count);
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    for id in 0..config.worker.worker_count {
        let channel = mq_channel.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            // Spawn consumer and return its JoinHandle

            let handle = spawn_worker(id, channel).await;
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    println!("Worker {} received shutdown signal", id);
                }

                res = handle => {
                    if let Err(e) = res {
                        eprintln!("Worker {} panicked: {:?}", id, e);
                    }
                }
            }

            println!("Worker {} stopped", id);
        });
    }

    // API Server
    let bind_addr = format!("{}:{}", config.api.host, config.api.port);
    println!("Starting HTTP server on {}", bind_addr);

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .app_data(web::Data::new(db_pool.clone()))
            .app_data(web::Data::new(mq_channel.clone()))
            .configure(health::config)
            .configure(publish::config)
            .configure(organization::config)
            .configure(day_map::config)
            .configure(processing_job::config)
    })
    .bind(&bind_addr)?
    .run();

    tokio::select! {
        res = server => res.map_err(|e| anyhow::anyhow!(e)),
        _ = tokio::signal::ctrl_c() => {
            println!("Shutdown signal received in main");
            Ok(())
        }
    }
    .map(|_| ())?;

    Ok(())
}

pub async fn spawn_worker(worker_id: usize, channel: lapin::Channel) -> JoinHandle<()> {
    //TODO: check queue type

    let mut consumer = Consumer::new(channel, QueueType::VideoProcessing)
        .await
        .expect("Failed to create consumer");

    consumer.set_handler(move |data, routing| handle_message(data.to_vec()));

    tokio::spawn(async move {
        if let Err(e) = consumer.start().await {
            eprintln!("[Worker {}] Consumer error: {}", worker_id, e);
        }
    })
}

fn handle_message(data: Vec<u8>) -> BoxFuture<'static, std::result::Result<(), anyhow::Error>> {
    Box::pin(async move {
        let payload = Payload::deserialize(&data)?;
        println!("{}", payload.id);

        Ok(())
    })
}
