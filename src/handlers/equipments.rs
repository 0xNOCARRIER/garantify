use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use axum_login::AuthSession;
use chrono::NaiveDate;
use sqlx::PgPool;
use tower_sessions::Session;
use tracing::error;
use uuid::Uuid;

use crate::{
    auth::backend::AuthBackend,
    config::Config,
    error::AppError,
    models::equipment::{compute_warranty_end, Equipment},
    services::{
        scrape::download_og_image,
        upload::{delete_equipment_files, save_image, save_pdf},
    },
    templates::{
        DashboardTemplate, DetailEquipmentTemplate, EditEquipmentTemplate, NewEquipmentTemplate,
        NotFoundTemplate,
    },
};

use super::auth::{check_csrf, new_csrf};

// -- Dashboard --

pub async fn dashboard(
    auth: AuthSession<AuthBackend>,
    State(pool): State<PgPool>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth.user.expect("login_required garantit un user");

    let equipments = sqlx::query_as::<_, Equipment>(
        "SELECT * FROM equipments WHERE user_id = $1 ORDER BY warranty_end_date ASC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;

    let total = equipments.len();
    let expiring_count = equipments
        .iter()
        .filter(|e| e.status_label() == "expiring")
        .count();
    let expired_count = equipments
        .iter()
        .filter(|e| e.status_label() == "expired")
        .count();

    Ok(DashboardTemplate {
        user_email: user.email,
        equipments,
        total,
        expiring_count,
        expired_count,
    })
}

// -- Nouveau équipement GET --

pub async fn new_equipment_page(
    _auth: AuthSession<AuthBackend>,
    session: Session,
    State(config): State<Config>,
) -> Result<impl IntoResponse, AppError> {
    Ok(NewEquipmentTemplate {
        csrf_token: new_csrf(&session).await?,
        error: None,
        name: String::new(),
        description: String::new(),
        purchase_date: String::new(),
        warranty_months: "24".into(),
        max_upload_mb: config.max_upload_mb,
    })
}

// -- Nouveau équipement POST --

pub async fn new_equipment_post(
    auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    State(config): State<Config>,
    multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user = auth.user.expect("login_required garantit un user");

    let form = parse_multipart(multipart, config.max_upload_mb).await?;

    if !check_csrf(&session, &form.csrf_token).await? {
        return Ok(Redirect::to("/equipments/new").into_response());
    }

    // Validation
    let csrf_token = new_csrf(&session).await?;
    macro_rules! form_err {
        ($msg:expr) => {
            return Ok(NewEquipmentTemplate {
                csrf_token,
                error: Some($msg.into()),
                name: form.name.clone(),
                description: form.description.clone(),
                purchase_date: form.purchase_date_str.clone(),
                warranty_months: form.warranty_months_str.clone(),
                max_upload_mb: config.max_upload_mb,
            }
            .into_response())
        };
    }

    if form.name.trim().is_empty() {
        form_err!("Le nom est obligatoire.");
    }
    if form.category.is_empty() {
        form_err!("La catégorie est obligatoire.");
    }
    if form.purchase_date_str.is_empty() {
        form_err!("La date d'achat est obligatoire.");
    }

    let purchase_date = NaiveDate::parse_from_str(&form.purchase_date_str, "%Y-%m-%d")
        .map_err(|_| ())
        .unwrap_or_default();
    if purchase_date == NaiveDate::default() {
        form_err!("Date d'achat invalide.");
    }

    let warranty_months: i32 = form.warranty_months_str.parse().unwrap_or(0);
    if warranty_months < 1 {
        form_err!("La durée de garantie doit être ≥ 1 mois.");
    }

    let warranty_end_date = compute_warranty_end(purchase_date, warranty_months);
    let equipment_id = Uuid::new_v4();

    // Upload image — priorité : fichier uploadé > image scrapée > rien
    let image_path = if let Some((data, ct)) = form.image {
        match save_image(data, &ct, &config.upload_dir, user.id, equipment_id).await {
            Ok(p) => Some(p),
            Err(e) => {
                error!("Upload image: {}", e);
                form_err!(format!("Image invalide : {}", e));
            }
        }
    } else if let Some(ref og_url) = form.scraped_image_url {
        // Télécharge l'image OG côté serveur (non-fatal si ça échoue)
        match download_og_image(og_url, &config.upload_dir, user.id, equipment_id).await {
            Ok(p) => Some(p),
            Err(e) => {
                error!("Download OG image: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Upload facture
    let invoice_path = if let Some((data, ct)) = form.invoice {
        match save_pdf(data, &ct, &config.upload_dir, user.id, equipment_id).await {
            Ok(p) => Some(p),
            Err(e) => {
                error!("Upload PDF: {}", e);
                form_err!(format!("Facture invalide : {}", e));
            }
        }
    } else {
        None
    };

    let product_url = form.product_url.filter(|u| !u.trim().is_empty());

    sqlx::query(
        "INSERT INTO equipments
         (id, user_id, name, description, category, purchase_type, product_url,
          image_path, invoice_path, purchase_date, warranty_months, warranty_end_date)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
    )
    .bind(equipment_id)
    .bind(user.id)
    .bind(form.name.trim())
    .bind({
        let trimmed = form.description.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
    .bind(&form.category)
    .bind(&form.purchase_type)
    .bind(&product_url)
    .bind(&image_path)
    .bind(&invoice_path)
    .bind(purchase_date)
    .bind(warranty_months)
    .bind(warranty_end_date)
    .execute(&pool)
    .await?;

    Ok(Redirect::to(&format!("/equipments/{}", equipment_id)).into_response())
}

// -- Détail --

pub async fn detail_equipment(
    auth: AuthSession<AuthBackend>,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth.user.expect("login_required garantit un user");

    let eq =
        sqlx::query_as::<_, Equipment>("SELECT * FROM equipments WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?;

    match eq {
        Some(eq) => Ok(DetailEquipmentTemplate { eq }.into_response()),
        None => Ok((
            StatusCode::NOT_FOUND,
            NotFoundTemplate {
                message: "Cet équipement n'existe pas ou ne vous appartient pas.",
            },
        )
            .into_response()),
    }
}

// -- Édition GET --

pub async fn edit_equipment_page(
    auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    State(config): State<Config>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth.user.expect("login_required garantit un user");

    let eq =
        sqlx::query_as::<_, Equipment>("SELECT * FROM equipments WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?;

    match eq {
        Some(eq) => Ok(EditEquipmentTemplate {
            csrf_token: new_csrf(&session).await?,
            eq,
            error: None,
            max_upload_mb: config.max_upload_mb,
        }
        .into_response()),
        None => Ok((
            StatusCode::NOT_FOUND,
            NotFoundTemplate {
                message: "Cet équipement n'existe pas ou ne vous appartient pas.",
            },
        )
            .into_response()),
    }
}

// -- Édition POST --

pub async fn edit_equipment_post(
    auth: AuthSession<AuthBackend>,
    session: Session,
    State(pool): State<PgPool>,
    State(config): State<Config>,
    Path(id): Path<Uuid>,
    multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    let user = auth.user.expect("login_required garantit un user");

    let existing =
        sqlx::query_as::<_, Equipment>("SELECT * FROM equipments WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&pool)
            .await?;

    let Some(existing) = existing else {
        return Ok(Redirect::to("/").into_response());
    };

    let form = parse_multipart(multipart, config.max_upload_mb).await?;

    if !check_csrf(&session, &form.csrf_token).await? {
        return Ok(Redirect::to(&format!("/equipments/{}/edit", id)).into_response());
    }

    let csrf_token = new_csrf(&session).await?;
    let eq_clone = existing.clone();
    macro_rules! edit_err {
        ($msg:expr) => {
            return Ok(EditEquipmentTemplate {
                csrf_token,
                eq: eq_clone.clone(),
                error: Some($msg.into()),
                max_upload_mb: config.max_upload_mb,
            }
            .into_response())
        };
    }

    if form.name.trim().is_empty() {
        edit_err!("Le nom est obligatoire.");
    }

    let purchase_date = NaiveDate::parse_from_str(&form.purchase_date_str, "%Y-%m-%d")
        .unwrap_or(existing.purchase_date);
    let warranty_months: i32 = form
        .warranty_months_str
        .parse()
        .unwrap_or(existing.warranty_months);
    if warranty_months < 1 {
        edit_err!("La durée de garantie doit être ≥ 1 mois.");
    }

    let warranty_end_date = compute_warranty_end(purchase_date, warranty_months);
    let product_url = form.product_url.filter(|u| !u.trim().is_empty());

    // Nouveaux fichiers s'ils sont fournis, sinon on garde les anciens
    let image_path = if let Some((data, ct)) = form.image {
        match save_image(data, &ct, &config.upload_dir, user.id, id).await {
            Ok(p) => Some(p),
            Err(e) => {
                edit_err!(format!("Image invalide : {}", e));
            }
        }
    } else {
        existing.image_path.clone()
    };

    let invoice_path = if let Some((data, ct)) = form.invoice {
        match save_pdf(data, &ct, &config.upload_dir, user.id, id).await {
            Ok(p) => Some(p),
            Err(e) => {
                edit_err!(format!("Facture invalide : {}", e));
            }
        }
    } else {
        existing.invoice_path.clone()
    };

    let description = {
        let trimmed = form.description.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    };

    sqlx::query(
        "UPDATE equipments SET
           name=$1, description=$2, category=$3, purchase_type=$4, product_url=$5,
           image_path=$6, invoice_path=$7, purchase_date=$8,
           warranty_months=$9, warranty_end_date=$10, updated_at=NOW()
         WHERE id=$11 AND user_id=$12",
    )
    .bind(form.name.trim())
    .bind(&description)
    .bind(&form.category)
    .bind(&form.purchase_type)
    .bind(&product_url)
    .bind(&image_path)
    .bind(&invoice_path)
    .bind(purchase_date)
    .bind(warranty_months)
    .bind(warranty_end_date)
    .bind(id)
    .bind(user.id)
    .execute(&pool)
    .await?;

    Ok(Redirect::to(&format!("/equipments/{}", id)).into_response())
}

// -- Suppression --

pub async fn delete_equipment(
    auth: AuthSession<AuthBackend>,
    State(pool): State<PgPool>,
    State(config): State<Config>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let user = auth.user.expect("login_required garantit un user");

    sqlx::query("DELETE FROM equipments WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user.id)
        .execute(&pool)
        .await?;

    // Nettoyage des fichiers uploadés
    delete_equipment_files(&config.upload_dir, user.id, id).await;

    Ok(Redirect::to("/"))
}

// -- Helper : parse multipart --

struct EquipmentFormData {
    csrf_token: String,
    name: String,
    description: String,
    category: String,
    purchase_type: String,
    product_url: Option<String>,
    purchase_date_str: String,
    warranty_months_str: String,
    image: Option<(Vec<u8>, String)>,
    invoice: Option<(Vec<u8>, String)>,
    scraped_image_url: Option<String>,
}

async fn parse_multipart(
    mut multipart: Multipart,
    max_upload_mb: u64,
) -> Result<EquipmentFormData, AppError> {
    let max_bytes = (max_upload_mb * 1024 * 1024) as usize;
    let mut form = EquipmentFormData {
        csrf_token: String::new(),
        name: String::new(),
        description: String::new(),
        category: String::new(),
        purchase_type: "online".into(),
        product_url: None,
        purchase_date_str: String::new(),
        warranty_months_str: "24".into(),
        image: None,
        invoice: None,
        scraped_image_url: None,
    };

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Multipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();
        let ct = field.content_type().unwrap_or("text/plain").to_string();
        let _is_file =
            !ct.starts_with("text/") && ct != "application/octet-stream" || ct == "application/pdf";

        match name.as_str() {
            "csrf_token" => form.csrf_token = field.text().await.unwrap_or_default(),
            "name" => form.name = field.text().await.unwrap_or_default(),
            "description" => form.description = field.text().await.unwrap_or_default(),
            "category" => form.category = field.text().await.unwrap_or_default(),
            "purchase_type" => form.purchase_type = field.text().await.unwrap_or_default(),
            "product_url" => form.product_url = Some(field.text().await.unwrap_or_default()),
            "scraped_image_url" => {
                let v = field.text().await.unwrap_or_default();
                if !v.is_empty() {
                    form.scraped_image_url = Some(v);
                }
            }
            "purchase_date" => form.purchase_date_str = field.text().await.unwrap_or_default(),
            "warranty_months" => form.warranty_months_str = field.text().await.unwrap_or_default(),
            "image" => {
                let data: Vec<u8> = field.bytes().await.unwrap_or_default().to_vec();
                if !data.is_empty() && data.len() <= max_bytes {
                    form.image = Some((data, ct));
                }
            }
            "invoice" => {
                let data: Vec<u8> = field.bytes().await.unwrap_or_default().to_vec();
                if !data.is_empty() && data.len() <= max_bytes {
                    form.invoice = Some((data, ct));
                }
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    Ok(form)
}
