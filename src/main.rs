use anyhow::Result;
use axum::{
    extract::FromRef,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use axum_login::AuthManagerLayerBuilder;
use std::net::SocketAddr;
use tower_http::services::ServeDir;
use tower_sessions::{cookie::SameSite, MemoryStore, SessionManagerLayer};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod auth;
mod config;
mod db;
mod error;
mod flash;
mod handlers;
mod jobs;
mod middleware;
mod models;
mod services;
mod templates;

use auth::backend::AuthBackend;
use config::Config;
use sqlx::PgPool;

#[derive(Debug, Clone)]
struct AppState {
    pool: PgPool,
    config: Config,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(s: &AppState) -> Self {
        s.pool.clone()
    }
}

impl FromRef<AppState> for Config {
    fn from_ref(s: &AppState) -> Self {
        s.config.clone()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cfg = config::Config::from_env()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&cfg.rust_log))
        .init();

    info!("Démarrage de warranty-tracker");

    let pool = db::create_pool(&cfg.database_url).await?;
    info!("Connexion PostgreSQL établie");

    sqlx::migrate!("./migrations").run(&pool).await?;
    info!("Migrations appliquées");

    let scheduler_pool = pool.clone();
    let scheduler_config = cfg.clone();
    tokio::spawn(async move {
        if let Err(e) = jobs::start_scheduler(scheduler_pool, scheduler_config).await {
            error!("Scheduler fatal: {}", e);
        }
    });

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_same_site(SameSite::Lax)
        .with_http_only(true);

    let auth_backend = AuthBackend::new(pool.clone());
    let auth_layer = AuthManagerLayerBuilder::new(auth_backend, session_layer).build();

    let upload_dir = cfg.upload_dir.clone();

    // Routes protégées (authentification requise)
    let password_limiter = middleware::new_password_limiter();
    let protected = Router::new()
        .route("/", get(handlers::dashboard))
        .route(
            "/equipments/new",
            get(handlers::equipments::new_equipment_page)
                .post(handlers::equipments::new_equipment_post),
        )
        .route(
            "/equipments/:id",
            get(handlers::equipments::detail_equipment),
        )
        .route(
            "/equipments/:id/edit",
            get(handlers::equipments::edit_equipment_page)
                .post(handlers::equipments::edit_equipment_post),
        )
        .route(
            "/equipments/:id/delete",
            post(handlers::equipments::delete_equipment),
        )
        .route(
            "/api/scrape-product",
            post(handlers::api::scrape_product_handler),
        )
        .route("/settings", get(handlers::settings::settings_page))
        .route("/settings/email", post(handlers::settings::settings_email))
        .route(
            "/settings/email/test",
            post(handlers::settings::settings_email_test),
        )
        .route("/settings/slack", post(handlers::settings::settings_slack))
        .route(
            "/settings/slack/test",
            post(handlers::settings::settings_slack_test),
        )
        // /settings/password a son propre rate limiter
        .route(
            "/settings/password",
            post(handlers::settings::settings_password).layer(
                axum::middleware::from_fn_with_state(password_limiter, middleware::rate_limit),
            ),
        )
        .route_layer(axum_login::login_required!(
            AuthBackend,
            login_url = "/login"
        ));

    let login_limiter = middleware::new_login_limiter();
    let auth_routes = Router::new()
        .route(
            "/login",
            get(handlers::auth::login_page).post(handlers::auth::login),
        )
        .route(
            "/register",
            get(handlers::auth::register_page).post(handlers::auth::register),
        )
        .layer(axum::middleware::from_fn_with_state(
            login_limiter,
            middleware::rate_limit,
        ));

    let app = Router::new()
        .merge(protected)
        .merge(auth_routes)
        .route("/logout", post(handlers::auth::logout))
        .route(
            "/password/forgot",
            get(handlers::auth::forgot_page).post(handlers::auth::forgot),
        )
        .route(
            "/password/reset",
            get(handlers::auth::reset_page).post(handlers::auth::reset),
        )
        .route("/health", get(|| async { StatusCode::OK }))
        .nest_service("/uploads", ServeDir::new(&upload_dir))
        .nest_service("/static", ServeDir::new("./static"))
        .fallback(handler_404)
        .layer(auth_layer)
        .with_state(AppState {
            pool,
            config: cfg.clone(),
        });

    let addr = format!("0.0.0.0:{}", cfg.port);
    info!("Écoute sur http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        templates::NotFoundTemplate {
            message: "Cette page n'existe pas.",
        },
    )
}
