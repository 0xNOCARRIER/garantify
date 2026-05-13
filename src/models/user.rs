use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // Paramètres de notification (migration 0005)
    pub notification_email: Option<String>,
    pub notifications_email_enabled: bool,
    pub slack_webhook_url: Option<String>, // chiffré AES-256-GCM en DB
    pub slack_notifications_enabled: bool,
}

impl axum_login::AuthUser for User {
    type Id = Uuid;

    fn id(&self) -> Self::Id {
        self.id
    }

    // Utilisé par axum-login pour invalider les sessions si le mot de passe change
    fn session_auth_hash(&self) -> &[u8] {
        self.password_hash.as_bytes()
    }
}
