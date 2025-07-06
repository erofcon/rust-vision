use crate::ProjectConfig;
use once_cell::sync::Lazy;
use std::sync::Arc;

pub static PROJECT_CONFIG: Lazy<Arc<ProjectConfig>> = Lazy::new(|| {
    let config = ProjectConfig::load().unwrap();

    // TODO: to add to log
    println!("Loaded project config");

    Arc::new(config)
});
