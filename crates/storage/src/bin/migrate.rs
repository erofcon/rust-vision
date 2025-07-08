use anyhow::{Error, Result};

use common::config::ProjectConfig;
use std::path::Path;
use storage::database::Database;

#[tokio::main]
async fn main() -> Result<(), Error> {
    unsafe {
        std::env::set_var("RUST_BACKTRACE", "full");
    }

    let config = ProjectConfig::load()?;

    let migrations_path = Path::new(&config.database.migrations_path);


    if !migrations_path.exists() {
        return Err(Error::msg("Migrations directory not found"));
    };

    let database = Database::new(&config.database.connection_string()).await?;

    database.migrate(migrations_path).await?;

    Ok(())
}
