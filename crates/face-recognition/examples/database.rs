use anyhow::Result;
use face_recognition::face_database::FaceDatabase;

#[tokio::main]
async fn main() -> Result<()> {
    let db_file = "assets/face_database.json";

    let mut db = FaceDatabase::new();

    db.load_from_folder("assets/face_database").await;

    Ok(())
}
