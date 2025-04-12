use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use anyhow::Result;

use api::handlers::{health, publish, video};
use common::config::{ApiConfig, CommonConfig};
use queue::connection::MQ;
use storage::database::Database;

#[actix_web::main]
async fn main() -> Result<()> {
    let api_config = ApiConfig::load()?;
    let common_config = CommonConfig::load()?;

    // TODO: check the correctness of copying in app data

    let db = Database::new(&common_config.database.connection_string()).await?;

    db.ping().await?;

    let db_pool = db.get_pool().clone();

    let mq = MQ::new("amqp://guest:guest@localhost:5672").await?;

    let mq_clone = mq.get_channel().clone();

    println!("Server running on {}:{}", api_config.host, api_config.port);

    HttpServer::new(move || {
        // Setting up CORS
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(Logger::default())
            .wrap(cors)
            .app_data(web::Data::new(db_pool.clone()))
            .app_data(web::Data::new(mq_clone.clone()))
            .configure(health::config)
            .configure(video::config)
            .configure(publish::config)
    })
    .bind(format!("{}:{}", api_config.host, api_config.port))?
    .run()
    .await?;

    Ok(())
}
