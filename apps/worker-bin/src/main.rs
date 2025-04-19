use anyhow::Result;
use common::config::WorkerConfig;

fn main() -> Result<()> {
    let config = WorkerConfig::load()?;

    println!("{:#?}", config.requeue_on_error);

    Ok(())
}
