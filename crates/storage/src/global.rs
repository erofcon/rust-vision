use crate::database::Database;
use project_config::global::PROJECT_CONFIG;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub static DATABASE: OnceCell<Arc<Database>> = OnceCell::const_new();

pub async fn get_database() -> &'static Arc<Database> {
    DATABASE
        .get_or_init(|| async {
            let connection_string = PROJECT_CONFIG.database.connection_string();
            let db = Database::new(&connection_string).await.unwrap();
            db.ping().await.unwrap();
            println!("Loaded database");
            Arc::new(db)
        })
        .await
}
