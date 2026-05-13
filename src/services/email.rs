use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use thiserror::Error;
use tracing::info;
use uuid::Uuid;

use crate::config::Config;

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("SMTP non configuré (SMTP_HOST / SMTP_USERNAME / SMTP_PASSWORD manquants)")]
    NotConfigured,
    #[error("Erreur de construction du message : {0}")]
    Build(#[from] lettre::error::Error),
    #[error("Erreur d'adresse email : {0}")]
    Address(#[from] lettre::address::AddressError),
    #[error("Erreur SMTP : {0}")]
    Smtp(#[from] lettre::transport::smtp::Error),
}

/// Résumé d'équipement utilisé dans les alertes et rapports mensuels.
pub struct EquipmentSummary {
    pub id: Uuid,
    pub name: String,
    pub warranty_end_date: chrono::NaiveDate,
}

pub async fn send_reset_email(config: &Config, to: &str, token: &str) -> Result<(), EmailError> {
    let base_url = config
        .app_base_url
        .as_deref()
        .unwrap_or("http://localhost:8080");
    let reset_url = format!("{}/password/reset?token={}", base_url, token);
    let html = format!(
        "<p>Bonjour,</p>\
        <p>Vous avez demandé la réinitialisation de votre mot de passe.</p>\
        <p><a href=\"{url}\">Réinitialiser mon mot de passe</a></p>\
        <p>Ce lien est valable <strong>1 heure</strong>.</p>\
        <p>Si vous n'avez pas fait cette demande, ignorez cet email.</p>",
        url = reset_url
    );
    send_email(
        config,
        to,
        "Réinitialisation de votre mot de passe — Garantify",
        html,
    )
    .await?;
    info!("Email de réinitialisation envoyé à {}", to);
    Ok(())
}

pub async fn send_email_test(config: &Config, to: &str) -> Result<(), EmailError> {
    let html = "<p>Bonjour,</p>\
        <p>Ceci est un email de test envoyé depuis <strong>Garantify</strong>.</p>\
        <p>Si vous recevez ce message, votre configuration email est correcte ✅</p>\
        <p style=\"color:#888;font-size:12px\">— Garantify</p>"
        .to_string();
    send_email(config, to, "Email de test — Garantify", html).await?;
    info!("Email de test envoyé à {}", to);
    Ok(())
}

pub async fn send_alert_email(
    config: &Config,
    to: &str,
    equipment: &EquipmentSummary,
    days_left: i64,
) -> Result<(), EmailError> {
    let base_url = config
        .app_base_url
        .as_deref()
        .unwrap_or("http://localhost:8080");
    let (subject, urgency) = match days_left {
        30 => (
            format!(
                "Rappel : garantie de « {} » expire dans 30 jours",
                equipment.name
            ),
            "expire dans <strong>30 jours</strong>",
        ),
        7 => (
            format!(
                "Urgent : garantie de « {} » expire dans 7 jours",
                equipment.name
            ),
            "expire dans <strong>7 jours</strong>",
        ),
        _ => (
            format!("Garantie de « {} » expirée aujourd'hui", equipment.name),
            "a expiré <strong>aujourd'hui</strong>",
        ),
    };

    let url = format!("{}/equipments/{}", base_url, equipment.id);
    let html = format!(
        "<p>Bonjour,</p>\
        <p>La garantie de votre équipement <strong>{name}</strong> {urgency} \
        (date de fin : {date}).</p>\
        <p><a href=\"{url}\">Voir l'équipement</a></p>\
        <p style=\"color:#888;font-size:12px\">— Garantify</p>",
        name = equipment.name,
        urgency = urgency,
        date = equipment.warranty_end_date,
        url = url,
    );

    send_email(config, to, &subject, html).await?;
    info!("Alerte garantie envoyée à {} pour «{}»", to, equipment.name);
    Ok(())
}

pub async fn send_monthly_report_email(
    config: &Config,
    to: &str,
    expiring_soon: &[EquipmentSummary],
    recently_expired: &[EquipmentSummary],
) -> Result<(), EmailError> {
    let base_url = config
        .app_base_url
        .as_deref()
        .unwrap_or("http://localhost:8080");
    let mut html =
        String::from("<p>Bonjour,</p><p>Voici votre récapitulatif mensuel Garantify.</p>");

    if !expiring_soon.is_empty() {
        html.push_str("<h3 style=\"color:#b45309\">Expirent dans les 30 prochains jours</h3><ul>");
        for eq in expiring_soon {
            html.push_str(&format!(
                "<li><a href=\"{base}/{id}\">{name}</a> — {date}</li>",
                base = base_url,
                id = eq.id,
                name = eq.name,
                date = eq.warranty_end_date,
            ));
        }
        html.push_str("</ul>");
    }

    if !recently_expired.is_empty() {
        html.push_str("<h3 style=\"color:#dc2626\">Expirées le mois dernier</h3><ul>");
        for eq in recently_expired {
            html.push_str(&format!(
                "<li><a href=\"{base}/{id}\">{name}</a> — {date}</li>",
                base = base_url,
                id = eq.id,
                name = eq.name,
                date = eq.warranty_end_date,
            ));
        }
        html.push_str("</ul>");
    }

    html.push_str(&format!(
        "<p><a href=\"{}\">Voir tous mes équipements</a></p>\
        <p style=\"color:#888;font-size:12px\">— Garantify</p>",
        base_url
    ));

    send_email(config, to, "Récapitulatif mensuel — Garantify", html).await?;
    info!("Rapport mensuel envoyé à {}", to);
    Ok(())
}

/// Envoie un email HTML via SMTP (lettre + native-tls).
async fn send_email(
    config: &Config,
    to: &str,
    subject: &str,
    html: String,
) -> Result<(), EmailError> {
    let (host, username, password, mail_from) = match (
        config.smtp_host.as_deref(),
        config.smtp_username.as_deref(),
        config.smtp_password.as_deref(),
        config.mail_from.as_deref(),
    ) {
        (Some(h), Some(u), Some(p), Some(f)) => (h, u, p, f),
        _ => return Err(EmailError::NotConfigured),
    };

    let from: Mailbox = format!("Garantify <{}>", mail_from).parse()?;
    let to_mailbox: Mailbox = to.parse()?;

    let message = Message::builder()
        .from(from)
        .to(to_mailbox)
        .subject(subject)
        .header(ContentType::TEXT_HTML)
        .body(html)?;

    let creds = Credentials::new(username.to_string(), password.to_string());

    let transport = if config.smtp_port == 587 {
        // STARTTLS
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?
            .credentials(creds)
            .build()
    } else {
        // SSL implicite (port 465 par défaut)
        AsyncSmtpTransport::<Tokio1Executor>::relay(host)?
            .port(config.smtp_port)
            .credentials(creds)
            .build()
    };

    transport.send(message).await?;
    Ok(())
}
