use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect},
    Form,
};
use axum_login::AuthSession;
use chrono::Utc;
use serde::Deserialize;
use sqlx::{PgPool, Row};
use tower_sessions::Session;
use tracing::{error, warn};
use uuid::Uuid;

use crate::{
    auth::backend::{AuthBackend, Credentials},
    config::Config,
    error::AppError,
    models::user::User,
    services::email::send_reset_email,
    templates::{ForgotPasswordTemplate, LoginTemplate, RegisterTemplate, ResetPasswordTemplate},
};

// -- Helpers CSRF --

pub async fn new_csrf(session: &Session) -> Result<String, AppError> {
    let token = Uuid::new_v4().to_string();
    session.insert("csrf_token", token.clone()).await?;
    Ok(token)
}

pub async fn check_csrf(session: &Session, form_token: &str) -> Result<bool, AppError> {
    let stored: Option<String> = session.get("csrf_token").await?;
    Ok(stored.as_deref() == Some(form_token))
}

// -- Register --

pub async fn register_page(session: Session) -> Result<impl IntoResponse, AppError> {
    Ok(RegisterTemplate {
        csrf_token: new_csrf(&session).await?,
        error: None,
    })
}

#[derive(Deserialize)]
pub struct RegisterForm {
    csrf_token: String,
    email: String,
    password: String,
    password_confirm: String,
}

pub async fn register(
    mut auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    Form(form): Form<RegisterForm>,
) -> Result<impl IntoResponse, AppError> {
    if !check_csrf(&session, &form.csrf_token).await? {
        warn!("CSRF invalide sur /register");
        return Ok(Redirect::to("/register").into_response());
    }

    let csrf_token = new_csrf(&session).await?;
    let email = form.email.trim().to_lowercase();

    if !email.contains('@') {
        return Ok(RegisterTemplate {
            csrf_token,
            error: Some("Email invalide.".into()),
        }
        .into_response());
    }
    if form.password.len() < 8 {
        return Ok(RegisterTemplate {
            csrf_token,
            error: Some("Le mot de passe doit contenir au moins 8 caractères.".into()),
        }
        .into_response());
    }
    if form.password != form.password_confirm {
        return Ok(RegisterTemplate {
            csrf_token,
            error: Some("Les mots de passe ne correspondent pas.".into()),
        }
        .into_response());
    }

    let password = form.password.clone();
    let hash = tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
    })
    .await
    .unwrap()
    .map_err(|_| AppError::Hash)?;

    let result = sqlx::query("INSERT INTO users (email, password_hash) VALUES ($1, $2)")
        .bind(&email)
        .bind(&hash)
        .execute(&pool)
        .await;

    match result {
        Ok(_) => {
            // Auto-login après inscription
            let creds = Credentials {
                email: email.clone(),
                password: form.password,
            };
            if let Ok(Some(user)) = auth.authenticate(creds).await {
                let _ = auth.login(&user).await;
            }
            Ok(Redirect::to("/").into_response())
        }
        Err(e) if is_unique_violation(&e) => Ok(RegisterTemplate {
            csrf_token,
            error: Some("Cet email est déjà utilisé.".into()),
        }
        .into_response()),
        Err(e) => Err(AppError::Db(e)),
    }
}

// -- Login --

pub async fn login_page(session: Session) -> Result<impl IntoResponse, AppError> {
    Ok(LoginTemplate {
        csrf_token: new_csrf(&session).await?,
        error: None,
    })
}

#[derive(Deserialize)]
pub struct LoginForm {
    csrf_token: String,
    email: String,
    password: String,
}

pub async fn login(
    mut auth: AuthSession<AuthBackend>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError> {
    if !check_csrf(&session, &form.csrf_token).await? {
        warn!("CSRF invalide sur /login");
        return Ok(Redirect::to("/login").into_response());
    }

    let creds = Credentials {
        email: form.email.trim().to_lowercase(),
        password: form.password,
    };

    match auth.authenticate(creds).await {
        Ok(Some(user)) => {
            auth.login(&user)
                .await
                .map_err(|e| AppError::Auth(e.to_string()))?;
            Ok(Redirect::to("/").into_response())
        }
        Ok(None) => {
            let csrf_token = new_csrf(&session).await?;
            Ok(LoginTemplate {
                csrf_token,
                error: Some("Email ou mot de passe incorrect.".into()),
            }
            .into_response())
        }
        Err(e) => {
            error!("Erreur backend auth : {}", e);
            let csrf_token = new_csrf(&session).await?;
            Ok(LoginTemplate {
                csrf_token,
                error: Some("Erreur interne, veuillez réessayer.".into()),
            }
            .into_response())
        }
    }
}

// -- Logout --

pub async fn logout(mut auth: AuthSession<AuthBackend>) -> Result<impl IntoResponse, AppError> {
    auth.logout()
        .await
        .map_err(|e| AppError::Auth(e.to_string()))?;
    Ok(Redirect::to("/login"))
}

// -- Forgot password --

pub async fn forgot_page(session: Session) -> Result<impl IntoResponse, AppError> {
    Ok(ForgotPasswordTemplate {
        csrf_token: new_csrf(&session).await?,
        success: false,
        error: None,
    })
}

#[derive(Deserialize)]
pub struct ForgotForm {
    csrf_token: String,
    email: String,
}

pub async fn forgot(
    session: Session,
    State(pool): State<PgPool>,
    State(config): State<Config>,
    Form(form): Form<ForgotForm>,
) -> Result<impl IntoResponse, AppError> {
    if !check_csrf(&session, &form.csrf_token).await? {
        return Ok(Redirect::to("/password/forgot").into_response());
    }

    let email = form.email.trim().to_lowercase();

    // On tente d'envoyer même si l'email n'existe pas (réponse identique dans les deux cas)
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(&email)
        .fetch_optional(&pool)
        .await?;

    if let Some(user) = user {
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + chrono::Duration::hours(1);

        // Purge des anciens tokens pour cet utilisateur
        sqlx::query("DELETE FROM password_reset_tokens WHERE user_id = $1")
            .bind(user.id)
            .execute(&pool)
            .await?;

        sqlx::query(
            "INSERT INTO password_reset_tokens (token, user_id, expires_at) VALUES ($1, $2, $3)",
        )
        .bind(&token)
        .bind(user.id)
        .bind(expires_at)
        .execute(&pool)
        .await?;

        if let Err(e) = send_reset_email(&config, &email, &token).await {
            let base_url = config
                .app_base_url
                .as_deref()
                .unwrap_or("http://localhost:8080");
            error!("Erreur envoi email reset : {}", e);
            warn!(
                "FALLBACK reset link pour {} : {}/password/reset?token={}",
                email, base_url, token
            );
        }
    }

    // Toujours la même réponse : ne pas révéler si l'email existe
    let csrf_token = new_csrf(&session).await?;
    Ok(ForgotPasswordTemplate {
        csrf_token,
        success: true,
        error: None,
    }
    .into_response())
}

// -- Reset password --

#[derive(Deserialize)]
pub struct ResetQuery {
    token: Option<String>,
}

pub async fn reset_page(
    session: Session,
    Query(query): Query<ResetQuery>,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, AppError> {
    let csrf_token = new_csrf(&session).await?;

    let Some(token) = query.token else {
        return Ok(ResetPasswordTemplate {
            csrf_token,
            token: String::new(),
            error: Some("Lien invalide ou expiré.".into()),
        }
        .into_response());
    };

    let valid =
        sqlx::query("SELECT 1 FROM password_reset_tokens WHERE token = $1 AND expires_at > NOW()")
            .bind(&token)
            .fetch_optional(&pool)
            .await?
            .is_some();

    if !valid {
        return Ok(ResetPasswordTemplate {
            csrf_token,
            token: String::new(),
            error: Some("Ce lien est invalide ou a expiré.".into()),
        }
        .into_response());
    }

    Ok(ResetPasswordTemplate {
        csrf_token,
        token,
        error: None,
    }
    .into_response())
}

#[derive(Deserialize)]
pub struct ResetForm {
    csrf_token: String,
    token: String,
    password: String,
    password_confirm: String,
}

pub async fn reset(
    session: Session,
    State(pool): State<PgPool>,
    Form(form): Form<ResetForm>,
) -> Result<impl IntoResponse, AppError> {
    if !check_csrf(&session, &form.csrf_token).await? {
        return Ok(Redirect::to("/login").into_response());
    }

    let csrf_token = new_csrf(&session).await?;

    if form.password.len() < 8 {
        return Ok(ResetPasswordTemplate {
            csrf_token,
            token: form.token,
            error: Some("Le mot de passe doit contenir au moins 8 caractères.".into()),
        }
        .into_response());
    }
    if form.password != form.password_confirm {
        return Ok(ResetPasswordTemplate {
            csrf_token,
            token: form.token,
            error: Some("Les mots de passe ne correspondent pas.".into()),
        }
        .into_response());
    }

    let row = sqlx::query(
        "SELECT user_id FROM password_reset_tokens WHERE token = $1 AND expires_at > NOW()",
    )
    .bind(&form.token)
    .fetch_optional(&pool)
    .await?;

    let Some(row) = row else {
        return Ok(ResetPasswordTemplate {
            csrf_token,
            token: String::new(),
            error: Some("Ce lien est invalide ou a expiré.".into()),
        }
        .into_response());
    };

    let user_id: Uuid = row.try_get("user_id").map_err(AppError::Db)?;

    let password = form.password.clone();
    let hash = tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
    })
    .await
    .unwrap()
    .map_err(|_| AppError::Hash)?;

    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(&hash)
        .bind(user_id)
        .execute(&pool)
        .await?;

    sqlx::query("DELETE FROM password_reset_tokens WHERE token = $1")
        .bind(&form.token)
        .execute(&pool)
        .await?;

    Ok(Redirect::to("/login").into_response())
}

// -- Utilitaires --

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}
