use reqwest::Client;
use serde_json::json;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum SlackError {
    #[error("URL de webhook invalide (doit commencer par https://hooks.slack.com/services/)")]
    InvalidUrl,
    #[error("Erreur réseau : {0}")]
    Http(#[from] reqwest::Error),
    #[error("Slack a rejeté le message (status {0})")]
    BadStatus(u16),
}

pub fn validate_webhook_url(url: &str) -> bool {
    url.starts_with("https://hooks.slack.com/services/")
}

/// Envoie un message texte sur un Incoming Webhook Slack.
pub async fn send_slack_message(webhook_url: &str, text: &str) -> Result<(), SlackError> {
    if !validate_webhook_url(webhook_url) {
        return Err(SlackError::InvalidUrl);
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let payload = json!({ "text": text });

    let resp = client.post(webhook_url).json(&payload).send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        error!("Slack webhook {} : {}", status.as_u16(), body);
        return Err(SlackError::BadStatus(status.as_u16()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_webhook_url() {
        let base = "https://hooks.slack.com/services/";
        let url = format!("{}{}/{}/{}", base, "T00000000", "B00000000", "X".repeat(24));
        assert!(validate_webhook_url(&url));
    }

    #[test]
    fn invalid_webhook_urls() {
        assert!(!validate_webhook_url("https://example.com/webhook"));
        assert!(!validate_webhook_url("http://hooks.slack.com/services/T/B/X"));
        assert!(!validate_webhook_url(""));
        assert!(!validate_webhook_url("hooks.slack.com/services/T/B/X"));
    }
}
