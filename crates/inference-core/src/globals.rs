use crate::model::Model;
use once_cell::sync::Lazy;
use project_config::global::PROJECT_CONFIG;
use std::sync::Arc;

pub static OBJECT_DETECTION_MODEL: Lazy<Arc<Model>> = Lazy::new(|| {
    let path = &PROJECT_CONFIG.object_detection_model.path;
    let model = Model::new(&path).expect("Failed to load object detection model");

    Arc::new(model)
});

pub static FACE_DETECTION_MODEL: Lazy<Arc<Model>> = Lazy::new(|| {
    let path = &PROJECT_CONFIG.face_detection_model.path;
    let model = Model::new(&path).expect("Failed to load face detection model");

    Arc::new(model)
});

pub static FACE_RECOGNITION_MODEL: Lazy<Arc<Model>> = Lazy::new(|| {
    let path = &PROJECT_CONFIG.face_recognition_model.path;
    let model = Model::new(&path).expect("Failed to load face recognition model");

    Arc::new(model)
});
