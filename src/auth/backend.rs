use async_trait::async_trait;
use axum_login::AuthnBackend;
use serde::Deserialize;
use sqlx::PgPool;
use thiserror::Error;

use crate::models::user::User;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Erreur base de données")]
    Db(#[from] sqlx::Error),
    #[error("Erreur de hachage")]
    Hash(argon2::password_hash::Error),
}

// argon2::password_hash::Error ne dérive pas std::error::Error,
// donc on implémente From manuellement au lieu d'utiliser #[from]
impl From<argon2::password_hash::Error> for AuthError {
    fn from(e: argon2::password_hash::Error) -> Self {
        AuthError::Hash(e)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct AuthBackend {
    pool: PgPool,
}

impl AuthBackend {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AuthnBackend for AuthBackend {
    type User = User;
    type Credentials = Credentials;
    type Error = AuthError;

    async fn authenticate(
        &self,
        creds: Self::Credentials,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(creds.email.to_lowercase())
            .fetch_optional(&self.pool)
            .await?;

        let Some(user) = user else {
            return Ok(None);
        };

        let hash = user.password_hash.clone();
        // verify_password est CPU-intensif — on le déplace sur le thread pool blocking
        let valid = tokio::task::spawn_blocking(move || {
            use argon2::{Argon2, PasswordHash, PasswordVerifier};
            PasswordHash::new(&hash)
                .and_then(|parsed| {
                    Argon2::default().verify_password(creds.password.as_bytes(), &parsed)
                })
                .is_ok()
        })
        .await
        .unwrap_or(false);

        Ok(if valid { Some(user) } else { None })
    }

    async fn get_user(
        &self,
        user_id: &axum_login::UserId<Self>,
    ) -> Result<Option<Self::User>, Self::Error> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }
}
