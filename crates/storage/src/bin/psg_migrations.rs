use sqlx::postgres::PgPool;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let migrations_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("migrations");

    println!("cargo:rerun-if-changed={}", migrations_path.display());

    let database_url = "postgres://myuser:mypassword@localhost:5432/myapp";
    let pool = PgPool::connect(&database_url).await?;

    let migrator = sqlx::migrate::Migrator::new(migrations_path.clone()).await?;
    migrator.run(&pool).await?;

    Ok(())
}
