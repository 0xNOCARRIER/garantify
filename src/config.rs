use std::env;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Variable d'environnement manquante : {0}")]
    Missing(String),
    #[error("Valeur invalide pour {var} : {msg}")]
    Invalid { var: String, msg: String },
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub upload_dir: String,
    pub max_upload_mb: u64,
    pub rust_log: String,
    pub _session_secret: Option<String>,
    pub mail_from: Option<String>,
    pub app_base_url: Option<String>,
    // SMTP
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    /// Clé AES-256 pour le chiffrement du webhook Slack (32 bytes).
    /// Obligatoire au démarrage. Générer avec : openssl rand -base64 32
    pub encryption_key: [u8; 32],
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let port = require("APP_PORT")?
            .parse::<u16>()
            .map_err(|_| ConfigError::Invalid {
                var: "APP_PORT".into(),
                msg: "doit être un numéro de port valide (1-65535)".into(),
            })?;

        let database_url = require("DATABASE_URL")?;

        let upload_dir = env::var("UPLOAD_DIR").unwrap_or_else(|_| "/data/uploads".into());

        let max_upload_mb = env::var("MAX_UPLOAD_MB")
            .unwrap_or_else(|_| "10".into())
            .parse::<u64>()
            .map_err(|_| ConfigError::Invalid {
                var: "MAX_UPLOAD_MB".into(),
                msg: "doit être un entier positif".into(),
            })?;

        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".into());

        let encryption_key = parse_encryption_key()?;

        let smtp_port = env::var("SMTP_PORT")
            .unwrap_or_else(|_| "465".into())
            .parse::<u16>()
            .map_err(|_| ConfigError::Invalid {
                var: "SMTP_PORT".into(),
                msg: "doit être un numéro de port valide".into(),
            })?;

        Ok(Self {
            port,
            database_url,
            upload_dir,
            max_upload_mb,
            rust_log,
            _session_secret: env::var("SESSION_SECRET").ok(),
            mail_from: env::var("MAIL_FROM").ok(),
            app_base_url: env::var("APP_BASE_URL").ok(),
            smtp_host: env::var("SMTP_HOST").ok(),
            smtp_port,
            smtp_username: env::var("SMTP_USERNAME").ok(),
            smtp_password: env::var("SMTP_PASSWORD").ok(),
            encryption_key,
        })
    }
}

fn parse_encryption_key() -> Result<[u8; 32], ConfigError> {
    use base64::Engine;
    let raw = require("ENCRYPTION_KEY")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw.trim())
        .map_err(|_| ConfigError::Invalid {
            var: "ENCRYPTION_KEY".into(),
            msg: "doit être une valeur base64 valide (openssl rand -base64 32)".into(),
        })?;
    bytes.try_into().map_err(|_| ConfigError::Invalid {
        var: "ENCRYPTION_KEY".into(),
        msg: "doit faire exactement 32 bytes après décodage base64".into(),
    })
}

fn require(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::Missing(key.into()))
}
