use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;

lazy_static::lazy_static! {
    pub static ref ACTIVE_PIPELINES: RwLock<HashMap<uuid::Uuid, Arc<AtomicBool>>> =
        RwLock::new(HashMap::new());
}
