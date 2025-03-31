use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use anyhow::Result;

use api::config::Config;
use api::handlers::{health, video};
use storage::config::DatabaseConfig;
use storage::database::Database;

#[actix_web::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;

    let db_config = DatabaseConfig {
        host: config.db_host.clone(),
        port: config.db_port,
        username: config.db_user.clone(),
        password: config.db_password.clone(),
        database_name: config.db_name.clone(),
    };

    let db = Database::new(&db_config.connection_string()).await?;

    let db_pool = db.get_pool().clone();

    println!("Server running on {}:{}", config.host, config.port);

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
            .configure(health::config)
            .configure(video::config)
    })
    .bind(format!("{}:{}", config.host, config.port))?
    .run()
    .await?;



    Ok(())
}
