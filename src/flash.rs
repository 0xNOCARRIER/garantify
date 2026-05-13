use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::error::AppError;

const FLASH_KEY: &str = "_flash";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flash {
    pub level: FlashLevel,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FlashLevel {
    Success,
    Error,
}

impl Flash {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Success,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: FlashLevel::Error,
            message: message.into(),
        }
    }

    pub fn is_success(&self) -> bool {
        self.level == FlashLevel::Success
    }
}

/// Stocke un flash message dans la session (écrase le précédent).
pub async fn set_flash(session: &Session, flash: Flash) -> Result<(), AppError> {
    session.insert(FLASH_KEY, flash).await?;
    Ok(())
}

/// Lit et supprime le flash message de la session.
pub async fn take_flash(session: &Session) -> Result<Option<Flash>, AppError> {
    let flash: Option<Flash> = session.remove(FLASH_KEY).await?;
    Ok(flash)
}
