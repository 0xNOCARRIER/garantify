use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Erreur base de données")]
    Db(#[from] sqlx::Error),
    #[error("Erreur de session")]
    Session(String),
    #[error("Erreur de hachage mot de passe")]
    Hash,
    #[error("Erreur d'authentification : {0}")]
    Auth(String),
    #[error("Erreur multipart : {0}")]
    Multipart(String),
}

impl From<tower_sessions::session::Error> for AppError {
    fn from(e: tower_sessions::session::Error) -> Self {
        AppError::Session(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Erreur serveur : {}", self);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Une erreur interne est survenue. Veuillez réessayer.",
        )
            .into_response()
    }
}
