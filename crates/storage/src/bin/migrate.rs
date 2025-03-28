use anyhow::{Error, Result};
use std::path::Path;
use storage::database::Database;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let database_url = "postgres://myuser:mypassword@localhost:5432/myapp";
    let migrations_path = &Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("migrations");

    let database = Database::new(database_url).await?;

    database.migrate(migrations_path).await?;

    Ok(())
}
