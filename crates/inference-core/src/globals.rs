use crate::face_database::FaceDatabase;
use crate::model::Model;
use once_cell::sync::Lazy;
use project_config::global::PROJECT_CONFIG;
use std::path::Path;
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

pub static FACE_DATABASE: Lazy<Arc<FaceDatabase>> = Lazy::new(|| {
    let db_file = &PROJECT_CONFIG.face_database.db_file;

    let face_database = if Path::new(db_file).exists() {
        let database = FaceDatabase::load_from_file().expect("Failed to load face_database");

        println!("Loaded face database from the file");
        Arc::new(database)
    } else {
        let mut db = FaceDatabase::new();

        db.load_from_folder().expect("Failed to load face_database");
        db.save_to_file().expect("Failed to save face_database");

        println!("Builded a database from folders");

        Arc::new(db)
    };

    face_database
});
