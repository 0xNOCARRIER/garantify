use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum UploadError {
    #[error("Type de fichier non supporté : {0}")]
    InvalidMime(String),
    #[error("Erreur IO : {0}")]
    Io(#[from] std::io::Error),
    #[error("Erreur image : {0}")]
    Image(#[from] image::ImageError),
}

/// Enregistre une image (jpeg/png/webp), resize si > 1920px de large, stocke en JPEG.
/// Retourne le chemin relatif depuis UPLOAD_DIR.
pub async fn save_image(
    data: Vec<u8>,
    content_type: &str,
    upload_dir: &str,
    user_id: Uuid,
    equipment_id: Uuid,
) -> Result<String, UploadError> {
    let allowed = ["image/jpeg", "image/jpg", "image/png", "image/webp"];
    if !allowed.contains(&content_type) {
        return Err(UploadError::InvalidMime(content_type.to_string()));
    }

    let dir = format!("{}/{}/{}", upload_dir, user_id, equipment_id);
    fs::create_dir_all(&dir).await?;

    let full_path = format!("{}/image.jpg", dir);
    let full_path_clone = full_path.clone();

    // Decode + resize dans un thread blocking (CPU-intensif)
    tokio::task::spawn_blocking(move || -> Result<(), UploadError> {
        let img = image::load_from_memory(&data)?;
        let img = if img.width() > 1920 {
            let scale = 1920.0 / img.width() as f32;
            let new_h = (img.height() as f32 * scale) as u32;
            img.resize(1920, new_h, image::imageops::FilterType::Lanczos3)
        } else {
            img
        };
        img.save_with_format(&full_path_clone, image::ImageFormat::Jpeg)?;
        Ok(())
    })
    .await
    .unwrap()?;

    Ok(format!("{}/{}/image.jpg", user_id, equipment_id))
}

/// Enregistre un PDF après vérification des magic bytes.
/// Retourne le chemin relatif depuis UPLOAD_DIR.
pub async fn save_pdf(
    data: Vec<u8>,
    content_type: &str,
    upload_dir: &str,
    user_id: Uuid,
    equipment_id: Uuid,
) -> Result<String, UploadError> {
    // Vérification double : Content-Type + magic bytes (%PDF-)
    if content_type != "application/pdf" && content_type != "application/x-pdf" {
        return Err(UploadError::InvalidMime(content_type.to_string()));
    }
    if !data.starts_with(b"%PDF-") {
        return Err(UploadError::InvalidMime("magic bytes PDF invalides".into()));
    }

    let dir = format!("{}/{}/{}", upload_dir, user_id, equipment_id);
    fs::create_dir_all(&dir).await?;

    let full_path = format!("{}/facture.pdf", dir);
    fs::write(&full_path, &data).await?;

    Ok(format!("{}/{}/facture.pdf", user_id, equipment_id))
}

/// Supprime le répertoire d'un équipement (appelé à la suppression).
pub async fn delete_equipment_files(upload_dir: &str, user_id: Uuid, equipment_id: Uuid) {
    let dir = format!("{}/{}/{}", upload_dir, user_id, equipment_id);
    let _ = fs::remove_dir_all(&dir).await;
}
