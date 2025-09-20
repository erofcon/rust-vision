use crate::ort_session::Model;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct FaceModels {
    pub detection: Arc<RwLock<Model>>,
    pub recognition: Arc<RwLock<Model>>,
}

pub struct Detectors {
    pub person: Option<Arc<RwLock<Model>>>,
    pub face: Option<FaceModels>,
}
