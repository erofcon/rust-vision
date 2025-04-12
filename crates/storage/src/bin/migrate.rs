use anyhow::{Error, Result};
use common::config::CommonConfig;
use std::path::Path;
use storage::database::Database;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let common_config = CommonConfig::load()?;

    let migrations_path = Path::new(&common_config.database.migrations_path);

    if !migrations_path.exists() {
        return Err(Error::msg("Migrations directory not found"));
    };

    let database = Database::new(&common_config.database.connection_string()).await?;

    database.migrate(migrations_path).await?;

    Ok(())
}
