use anyhow::Result;
use vision::face_database::FaceDatabase;

#[tokio::main]
async fn main() -> Result<()> {
    let db_file = "assets/face_database.json";

    let mut db = FaceDatabase::new();
    db.load_from_folder().await?;

    db.save_to_file(db_file)?;

    Ok(())
}
