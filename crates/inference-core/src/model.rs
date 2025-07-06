use anyhow::Result;
use ort::execution_providers::{CPUExecutionProvider, CUDAExecutionProvider};
use ort::session::Session;

pub struct Model {
    session: Session,
}

impl Model {
    // TODO: add logging
    pub fn new(path: &str) -> Result<Self> {
        ort::init()
            .with_execution_providers([
                CUDAExecutionProvider::default().build().error_on_failure(),
                CPUExecutionProvider::default().build().error_on_failure(),
            ])
            .commit()?;

        let session = Session::builder()?
            .with_inter_threads(4)?
            .commit_from_file(path)?;

        println!("Loaded model from: {}", path);

        Ok(Model { session })
    }

    pub fn get_session(&self) -> &Session {
        &self.session
    }
}
