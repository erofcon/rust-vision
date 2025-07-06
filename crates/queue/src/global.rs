use crate::connection::MQ;
use project_config::global::PROJECT_CONFIG;
use std::sync::Arc;
use tokio::sync::OnceCell;

pub static MQ: OnceCell<Arc<MQ>> = OnceCell::const_new();

pub async fn get_mq() -> &'static Arc<MQ> {
    MQ.get_or_init(|| async {
        let connection_string = PROJECT_CONFIG.mq.connection_to_string();
        let mq = MQ::new(&connection_string).await.unwrap();
        println!("Loaded MQ");
        Arc::new(mq)
    })
    .await
}
