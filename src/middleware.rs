use std::{
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::Arc,
};

use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};

pub type RateLimiterState = Arc<DefaultKeyedRateLimiter<IpAddr>>;

/// Crée un rate limiter : 5 requêtes par minute par IP (login/register).
pub fn new_login_limiter() -> RateLimiterState {
    let quota = Quota::per_minute(NonZeroU32::new(5).expect("5 > 0"));
    Arc::new(RateLimiter::keyed(quota))
}

/// Crée un rate limiter : 5 requêtes par 15 minutes par IP (changement mot de passe).
pub fn new_password_limiter() -> RateLimiterState {
    use std::time::Duration;
    let quota = Quota::with_period(Duration::from_secs(900))
        .expect("durée valide")
        .allow_burst(NonZeroU32::new(5).expect("5 > 0"));
    Arc::new(RateLimiter::keyed(quota))
}

/// Middleware axum : bloque avec 429 si l'IP dépasse le quota.
pub async fn rate_limit(
    State(limiter): State<RateLimiterState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let ip = addr.ip();
    match limiter.check_key(&ip) {
        Ok(()) => next.run(req).await,
        Err(_) => (
            StatusCode::TOO_MANY_REQUESTS,
            "Trop de tentatives. Réessayez dans une minute.",
        )
            .into_response(),
    }
}
