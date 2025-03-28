use anyhow::{Context, Result};
use sqlx::pool::PoolOptions;
use sqlx::{Pool, Postgres};
use std::path::Path;

pub struct Database {
    pool: Pool<Postgres>,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool_options = PoolOptions::new();

        let pool = pool_options
            .connect(database_url)
            .await
            .context("Failed to connect to database")?;

        Ok(Database { pool })
    }

    pub async fn migrate(&self, migration_path: &Path) -> Result<()> {
        let migrator = sqlx::migrate::Migrator::new(migration_path).await?;
        migrator.run(&self.pool).await?;

        Ok(())
    }

    pub fn get_pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

    pub async fn ping(&self) -> Result<()> {
        self.pool
            .acquire()
            .await
            .context("Failed to establish connection to database")?;

        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}
