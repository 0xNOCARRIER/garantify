use axum::{response::IntoResponse, Json};
use axum_login::AuthSession;
use serde::{Deserialize, Serialize};

use crate::{auth::backend::AuthBackend, services::scrape::scrape_product};

#[derive(Deserialize)]
pub struct ScrapeRequest {
    url: String,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum ScrapeResponse {
    Ok {
        name: Option<String>,
        description: Option<String>,
        image_url: Option<String>,
    },
    Err {
        error: String,
    },
}

/// POST /api/scrape-product
/// Route protégée — uniquement accessible à un utilisateur connecté.
pub async fn scrape_product_handler(
    _auth: AuthSession<AuthBackend>,
    Json(body): Json<ScrapeRequest>,
) -> impl IntoResponse {
    match scrape_product(&body.url).await {
        Ok(r) => Json(ScrapeResponse::Ok {
            name: r.name,
            description: r.description,
            image_url: r.image_url,
        }),
        Err(e) => Json(ScrapeResponse::Err {
            error: e.to_string(),
        }),
    }
}
