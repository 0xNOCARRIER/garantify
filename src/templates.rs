use crate::flash::Flash;
use crate::models::equipment::Equipment;
use askama::Template;

// -- Auth --

#[derive(Template)]
#[template(path = "auth/login.html")]
pub struct LoginTemplate {
    pub csrf_token: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/register.html")]
pub struct RegisterTemplate {
    pub csrf_token: String,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/forgot_password.html")]
pub struct ForgotPasswordTemplate {
    pub csrf_token: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "auth/reset_password.html")]
pub struct ResetPasswordTemplate {
    pub csrf_token: String,
    pub token: String,
    pub error: Option<String>,
}

// -- Dashboard --

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub user_email: String,
    pub equipments: Vec<Equipment>,
    pub total: usize,
    pub expiring_count: usize,
    pub expired_count: usize,
}

// -- Settings --

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub csrf_token: String,
    pub flash: Option<Flash>,
    pub user_email: String,
    pub notification_email: String,
    pub notifications_email_enabled: bool,
    pub slack_configured: bool,
    pub slack_notifications_enabled: bool,
    pub effective_notification_email: String,
}

// -- Erreurs --

#[derive(Template)]
#[template(path = "404.html")]
pub struct NotFoundTemplate {
    pub message: &'static str,
}

// -- Équipements --

#[derive(Template)]
#[template(path = "equipments/new.html")]
pub struct NewEquipmentTemplate {
    pub csrf_token: String,
    pub error: Option<String>,
    // Valeurs à re-remplir si erreur de validation
    pub name: String,
    pub description: String,
    pub purchase_date: String,
    pub warranty_months: String,
    pub max_upload_mb: u64,
}

#[derive(Template)]
#[template(path = "equipments/detail.html")]
pub struct DetailEquipmentTemplate {
    pub eq: Equipment,
}

#[derive(Template)]
#[template(path = "equipments/edit.html")]
pub struct EditEquipmentTemplate {
    pub csrf_token: String,
    pub eq: Equipment,
    pub error: Option<String>,
    pub max_upload_mb: u64,
}
