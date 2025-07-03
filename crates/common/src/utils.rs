use crate::ort_session::Model;

pub struct FaceModels {
    pub detection: Model,
    pub recognition: Model,
}

pub struct Detectors {
    pub person: Option<Model>,
    pub face: Option<FaceModels>,
}
