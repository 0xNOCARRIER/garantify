use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Form,
};
use axum_login::AuthSession;
use serde::Deserialize;
use sqlx::PgPool;
use tower_sessions::Session;
use tracing::{error, warn};

use crate::{
    auth::backend::AuthBackend,
    config::Config,
    error::AppError,
    flash::{set_flash, take_flash, Flash},
    models::user::User,
    services::{
        crypto::{decrypt, encrypt},
        email::send_email_test,
        slack::{send_slack_message, validate_webhook_url},
    },
    templates::SettingsTemplate,
};

// -- GET /settings --

pub async fn settings_page(
    auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth.user.expect("login_required garantit un user");

    // Recharger depuis la DB pour avoir les champs à jour
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&pool)
        .await?;

    let flash = take_flash(&session).await?;
    let csrf_token = crate::handlers::auth::new_csrf(&session).await?;

    // Masquer le webhook : on indique seulement s'il est configuré
    let slack_configured = user.slack_webhook_url.is_some();

    // Email de notification effectif
    let effective_notification_email = user
        .notification_email
        .clone()
        .unwrap_or_else(|| user.email.clone());

    Ok(SettingsTemplate {
        csrf_token,
        flash,
        user_email: user.email,
        notification_email: user.notification_email.unwrap_or_default(),
        notifications_email_enabled: user.notifications_email_enabled,
        slack_configured,
        slack_notifications_enabled: user.slack_notifications_enabled,
        effective_notification_email,
    })
}

// -- POST /settings/password --

#[derive(Deserialize)]
pub struct PasswordForm {
    csrf_token: String,
    current_password: String,
    new_password: String,
    new_password_confirm: String,
}

pub async fn settings_password(
    mut auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    Form(form): Form<PasswordForm>,
) -> Result<impl IntoResponse, AppError> {
    if !crate::handlers::auth::check_csrf(&session, &form.csrf_token).await? {
        warn!("CSRF invalide sur /settings/password");
        return Ok(Redirect::to("/settings").into_response());
    }

    let user = auth.user.clone().expect("login_required garantit un user");

    // Vérifier le mot de passe actuel (CPU-intensif → thread pool)
    let hash = user.password_hash.clone();
    let current_ok = tokio::task::spawn_blocking({
        let pwd = form.current_password.clone();
        move || {
            PasswordHash::new(&hash)
                .and_then(|h| Argon2::default().verify_password(pwd.as_bytes(), &h))
                .is_ok()
        }
    })
    .await
    .unwrap_or(false);

    if !current_ok {
        set_flash(&session, Flash::error("Mot de passe actuel incorrect.")).await?;
        return Ok(Redirect::to("/settings").into_response());
    }

    if form.new_password.len() < 8 {
        set_flash(
            &session,
            Flash::error("Le nouveau mot de passe doit contenir au moins 8 caractères."),
        )
        .await?;
        return Ok(Redirect::to("/settings").into_response());
    }

    if form.new_password != form.new_password_confirm {
        set_flash(&session, Flash::error("Les nouveaux mots de passe ne correspondent pas."))
            .await?;
        return Ok(Redirect::to("/settings").into_response());
    }

    let new_hash = tokio::task::spawn_blocking({
        let pwd = form.new_password.clone();
        move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(pwd.as_bytes(), &salt)
                .map(|h| h.to_string())
        }
    })
    .await
    .unwrap()
    .map_err(|_| AppError::Hash)?;

    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(&new_hash)
        .bind(user.id)
        .execute(&pool)
        .await?;

    // La session sera invalidée automatiquement par axum-login (session_auth_hash),
    // mais on force le logout immédiatement pour la clarté UX.
    auth.logout().await.map_err(|e| AppError::Auth(e.to_string()))?;

    Ok(Redirect::to("/login?changed=1").into_response())
}

// -- POST /settings/email --

#[derive(Deserialize)]
pub struct EmailSettingsForm {
    csrf_token: String,
    notification_email: String,
    #[serde(default)]
    notifications_email_enabled: Option<String>, // checkbox : Some("on") ou None
}

pub async fn settings_email(
    auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    Form(form): Form<EmailSettingsForm>,
) -> Result<impl IntoResponse, AppError> {
    if !crate::handlers::auth::check_csrf(&session, &form.csrf_token).await? {
        warn!("CSRF invalide sur /settings/email");
        return Ok(Redirect::to("/settings").into_response());
    }

    let user = auth.user.expect("login_required garantit un user");

    let notification_email: Option<String> = {
        let trimmed = form.notification_email.trim();
        if trimmed.is_empty() {
            None
        } else if !trimmed.contains('@') {
            set_flash(&session, Flash::error("Adresse email de notification invalide.")).await?;
            return Ok(Redirect::to("/settings").into_response());
        } else {
            Some(trimmed.to_lowercase())
        }
    };

    let enabled = form.notifications_email_enabled.as_deref() == Some("on");

    sqlx::query(
        "UPDATE users SET notification_email = $1, notifications_email_enabled = $2, updated_at = NOW() WHERE id = $3",
    )
    .bind(&notification_email)
    .bind(enabled)
    .bind(user.id)
    .execute(&pool)
    .await?;

    set_flash(&session, Flash::success("Paramètres email mis à jour.")).await?;
    Ok(Redirect::to("/settings").into_response())
}

// -- POST /settings/email/test --

#[derive(Deserialize)]
pub struct EmailTestForm {
    csrf_token: String,
}

pub async fn settings_email_test(
    auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    State(config): State<Config>,
    Form(form): Form<EmailTestForm>,
) -> Result<impl IntoResponse, AppError> {
    if !crate::handlers::auth::check_csrf(&session, &form.csrf_token).await? {
        return Ok(Redirect::to("/settings").into_response());
    }

    let user = auth.user.expect("login_required garantit un user");

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&pool)
        .await?;

    let to = user.notification_email.as_deref().unwrap_or(&user.email);

    match send_email_test(&config, to).await {
        Ok(_) => set_flash(&session, Flash::success(format!("Email de test envoyé à {to}."))).await?,
        Err(e) => {
            error!("Erreur email de test : {}", e);
            set_flash(&session, Flash::error(format!("Échec de l'envoi : {e}"))).await?;
        }
    }

    Ok(Redirect::to("/settings").into_response())
}

// -- POST /settings/slack --

#[derive(Deserialize)]
pub struct SlackSettingsForm {
    csrf_token: String,
    slack_webhook_url: String,
    #[serde(default)]
    slack_notifications_enabled: Option<String>,
}

pub async fn settings_slack(
    auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    State(config): State<Config>,
    Form(form): Form<SlackSettingsForm>,
) -> Result<impl IntoResponse, AppError> {
    if !crate::handlers::auth::check_csrf(&session, &form.csrf_token).await? {
        warn!("CSRF invalide sur /settings/slack");
        return Ok(Redirect::to("/settings").into_response());
    }

    let user = auth.user.expect("login_required garantit un user");
    let enabled = form.slack_notifications_enabled.as_deref() == Some("on");

    let encrypted_url: Option<String> = {
        let trimmed = form.slack_webhook_url.trim();
        if trimmed.is_empty() {
            // Champ vide = supprimer le webhook
            None
        } else if !validate_webhook_url(trimmed) {
            set_flash(
                &session,
                Flash::error("URL invalide. Elle doit commencer par https://hooks.slack.com/services/"),
            )
            .await?;
            return Ok(Redirect::to("/settings").into_response());
        } else {
            match encrypt(&config.encryption_key, trimmed) {
                Ok(enc) => Some(enc),
                Err(e) => {
                    error!("Erreur chiffrement webhook : {}", e);
                    set_flash(&session, Flash::error("Erreur interne lors du chiffrement.")).await?;
                    return Ok(Redirect::to("/settings").into_response());
                }
            }
        }
    };

    sqlx::query(
        "UPDATE users SET slack_webhook_url = $1, slack_notifications_enabled = $2, updated_at = NOW() WHERE id = $3",
    )
    .bind(&encrypted_url)
    .bind(enabled)
    .bind(user.id)
    .execute(&pool)
    .await?;

    set_flash(&session, Flash::success("Paramètres Slack mis à jour.")).await?;
    Ok(Redirect::to("/settings").into_response())
}

// -- POST /settings/slack/test --

#[derive(Deserialize)]
pub struct SlackTestForm {
    csrf_token: String,
}

pub async fn settings_slack_test(
    auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    State(config): State<Config>,
    Form(form): Form<SlackTestForm>,
) -> Result<impl IntoResponse, AppError> {
    if !crate::handlers::auth::check_csrf(&session, &form.csrf_token).await? {
        return Ok(Redirect::to("/settings").into_response());
    }

    let user = auth.user.expect("login_required garantit un user");

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_one(&pool)
        .await?;

    let Some(encrypted) = &user.slack_webhook_url else {
        set_flash(&session, Flash::error("Aucun webhook Slack configuré.")).await?;
        return Ok(Redirect::to("/settings").into_response());
    };

    let webhook_url = match decrypt(&config.encryption_key, encrypted) {
        Ok(url) => url,
        Err(e) => {
            error!("Erreur déchiffrement webhook : {}", e);
            set_flash(&session, Flash::error("Impossible de lire le webhook — clé de chiffrement incorrecte ?")).await?;
            return Ok(Redirect::to("/settings").into_response());
        }
    };

    match send_slack_message(&webhook_url, "✅ *Garantify* — test de connexion réussi !").await {
        Ok(_) => set_flash(&session, Flash::success("Message de test envoyé sur Slack !")).await?,
        Err(e) => {
            error!("Erreur test Slack : {}", e);
            set_flash(&session, Flash::error(format!("Échec : {e}"))).await?;
        }
    }

    Ok(Redirect::to("/settings").into_response())
}
