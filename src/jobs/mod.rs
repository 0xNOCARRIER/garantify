use chrono::{Datelike, Local, NaiveDate};
use sqlx::PgPool;
use thiserror::Error;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    config::Config,
    services::{
        crypto::decrypt,
        email::{send_alert_email, send_monthly_report_email, EmailError, EquipmentSummary},
        slack::send_slack_message,
    },
};

#[derive(Debug, Error)]
pub enum JobError {
    #[error("Scheduler : {0}")]
    Scheduler(String),
    #[error("Base de données : {0}")]
    Db(#[from] sqlx::Error),
    #[error("Calcul de date invalide")]
    Date,
}

#[derive(sqlx::FromRow)]
struct AlertRow {
    equipment_id: Uuid,
    user_id: Uuid,
    name: String,
    user_email: String,
    notification_email: Option<String>,
    notifications_email_enabled: bool,
    slack_webhook_url: Option<String>,
    slack_notifications_enabled: bool,
    warranty_end_date: NaiveDate,
}

#[derive(sqlx::FromRow)]
struct EquipmentRow {
    id: Uuid,
    name: String,
    warranty_end_date: NaiveDate,
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    notification_email: Option<String>,
    notifications_email_enabled: bool,
    slack_webhook_url: Option<String>,
    slack_notifications_enabled: bool,
}

pub async fn start_scheduler(pool: PgPool, config: Config) -> Result<(), JobError> {
    let sched = JobScheduler::new()
        .await
        .map_err(|e| JobError::Scheduler(e.to_string()))?;

    let pool1 = pool.clone();
    let cfg1 = config.clone();
    sched
        .add(
            Job::new_async("0 0 7 * * *", move |_uuid, _lock| {
                let pool = pool1.clone();
                let cfg = cfg1.clone();
                Box::pin(async move {
                    info!("Cron: alertes quotidiennes");
                    if let Err(e) = run_daily_alerts(&pool, &cfg).await {
                        error!("daily_alerts: {}", e);
                    }
                })
            })
            .map_err(|e| JobError::Scheduler(e.to_string()))?,
        )
        .await
        .map_err(|e| JobError::Scheduler(e.to_string()))?;

    let pool2 = pool.clone();
    let cfg2 = config.clone();
    sched
        .add(
            Job::new_async("0 0 8 1 * *", move |_uuid, _lock| {
                let pool = pool2.clone();
                let cfg = cfg2.clone();
                Box::pin(async move {
                    info!("Cron: rapport mensuel");
                    if let Err(e) = run_monthly_report(&pool, &cfg).await {
                        error!("monthly_report: {}", e);
                    }
                })
            })
            .map_err(|e| JobError::Scheduler(e.to_string()))?,
        )
        .await
        .map_err(|e| JobError::Scheduler(e.to_string()))?;

    sched
        .start()
        .await
        .map_err(|e| JobError::Scheduler(e.to_string()))?;

    info!("Scheduler démarré (alertes 07:00 UTC / rapport mensuel le 1er 08:00 UTC)");
    std::future::pending::<()>().await;
    Ok(())
}

async fn run_daily_alerts(pool: &PgPool, config: &Config) -> Result<(), JobError> {
    let today = Local::now().date_naive();

    let rows = sqlx::query_as::<_, AlertRow>(
        "SELECT e.id AS equipment_id, e.user_id, e.name,
                u.email AS user_email,
                u.notification_email, u.notifications_email_enabled,
                u.slack_webhook_url, u.slack_notifications_enabled,
                e.warranty_end_date
         FROM equipments e
         JOIN users u ON u.id = e.user_id
         WHERE e.warranty_end_date IN ($1, $2, $3)",
    )
    .bind(today + chrono::Duration::days(30))
    .bind(today + chrono::Duration::days(7))
    .bind(today)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let days_left = (row.warranty_end_date - today).num_days();
        let kind = match days_left {
            30 => "alert_30d",
            7  => "alert_7d",
            0  => "alert_expired",
            _  => continue,
        };

        let eq = EquipmentSummary {
            id: row.equipment_id,
            name: row.name.clone(),
            warranty_end_date: row.warranty_end_date,
        };

        // ── Canal email ──────────────────────────────────────────────
        if row.notifications_email_enabled && smtp_configured(config) {
            let already: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM notifications_sent
                 WHERE equipment_id = $1 AND kind = $2 AND channel = 'email')",
            )
            .bind(row.equipment_id)
            .bind(kind)
            .fetch_one(pool)
            .await?;

            if !already {
                let to = row.notification_email.as_deref().unwrap_or(&row.user_email);
                match send_alert_email(config, to, &eq, days_left).await {
                    Ok(()) => {
                        sqlx::query(
                            "INSERT INTO notifications_sent (user_id, equipment_id, kind, channel)
                             VALUES ($1, $2, $3, 'email') ON CONFLICT DO NOTHING",
                        )
                        .bind(row.user_id)
                        .bind(row.equipment_id)
                        .bind(kind)
                        .execute(pool)
                        .await?;
                    }
                    Err(e) => error!("Alerte email {} «{}» : {}", to, row.name, e),
                }
            }
        }

        // ── Canal Slack ──────────────────────────────────────────────
        if row.slack_notifications_enabled {
            if let Some(encrypted) = &row.slack_webhook_url {
                let already: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM notifications_sent
                     WHERE equipment_id = $1 AND kind = $2 AND channel = 'slack')",
                )
                .bind(row.equipment_id)
                .bind(kind)
                .fetch_one(pool)
                .await?;

                if !already {
                    match decrypt(&config.encryption_key, encrypted) {
                        Ok(url) => {
                            let emoji = match days_left {
                                30 => "📅",
                                7  => "⚠️",
                                _  => "❌",
                            };
                            let text = format!(
                                "{emoji} *Garantify* — garantie de *{}* expire dans *{} j.* ({})",
                                row.name, days_left, row.warranty_end_date
                            );
                            match send_slack_message(&url, &text).await {
                                Ok(()) => {
                                    sqlx::query(
                                        "INSERT INTO notifications_sent (user_id, equipment_id, kind, channel)
                                         VALUES ($1, $2, $3, 'slack') ON CONFLICT DO NOTHING",
                                    )
                                    .bind(row.user_id)
                                    .bind(row.equipment_id)
                                    .bind(kind)
                                    .execute(pool)
                                    .await?;
                                }
                                Err(e) => error!("Alerte Slack «{}» : {}", row.name, e),
                            }
                        }
                        Err(e) => error!("Déchiffrement webhook Slack : {}", e),
                    }
                }
            }
        }
    }

    Ok(())
}

async fn run_monthly_report(pool: &PgPool, config: &Config) -> Result<(), JobError> {
    let today = Local::now().date_naive();
    let current_month = today.format("%Y-%m").to_string();

    let first_of_this_month =
        NaiveDate::from_ymd_opt(today.year(), today.month(), 1).ok_or(JobError::Date)?;
    let last_of_prev_month = first_of_this_month.pred_opt().ok_or(JobError::Date)?;
    let first_of_prev_month =
        NaiveDate::from_ymd_opt(last_of_prev_month.year(), last_of_prev_month.month(), 1)
            .ok_or(JobError::Date)?;

    let users = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, notification_email, notifications_email_enabled,
                slack_webhook_url, slack_notifications_enabled
         FROM users",
    )
    .fetch_all(pool)
    .await?;

    for user in users {
        let expiring = sqlx::query_as::<_, EquipmentRow>(
            "SELECT id, name, warranty_end_date FROM equipments
             WHERE user_id = $1 AND warranty_end_date BETWEEN $2 AND $3
             ORDER BY warranty_end_date ASC",
        )
        .bind(user.id)
        .bind(today)
        .bind(today + chrono::Duration::days(30))
        .fetch_all(pool)
        .await?;

        let recently_expired = sqlx::query_as::<_, EquipmentRow>(
            "SELECT id, name, warranty_end_date FROM equipments
             WHERE user_id = $1 AND warranty_end_date BETWEEN $2 AND $3
             ORDER BY warranty_end_date DESC",
        )
        .bind(user.id)
        .bind(first_of_prev_month)
        .bind(last_of_prev_month)
        .fetch_all(pool)
        .await?;

        if expiring.is_empty() && recently_expired.is_empty() {
            continue;
        }

        let to_summaries = |rows: Vec<EquipmentRow>| -> Vec<EquipmentSummary> {
            rows.into_iter()
                .map(|r| EquipmentSummary { id: r.id, name: r.name, warranty_end_date: r.warranty_end_date })
                .collect()
        };
        let expiring_s = to_summaries(expiring);
        let expired_s  = to_summaries(recently_expired);

        // ── Canal email ──────────────────────────────────────────────
        if user.notifications_email_enabled && smtp_configured(config) {
            let already: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM notifications_sent
                 WHERE user_id = $1 AND month = $2 AND kind = 'monthly_report' AND channel = 'email')",
            )
            .bind(user.id)
            .bind(&current_month)
            .fetch_one(pool)
            .await?;

            if !already {
                let to = user.notification_email.as_deref().unwrap_or(&user.email);
                match send_monthly_report_email(config, to, &expiring_s, &expired_s).await {
                    Ok(()) => {
                        sqlx::query(
                            "INSERT INTO notifications_sent (user_id, kind, month, channel)
                             VALUES ($1, 'monthly_report', $2, 'email') ON CONFLICT DO NOTHING",
                        )
                        .bind(user.id)
                        .bind(&current_month)
                        .execute(pool)
                        .await?;
                    }
                    Err(e) => error!("Rapport email {} : {}", user.email, e),
                }
            }
        }

        // ── Canal Slack ──────────────────────────────────────────────
        if user.slack_notifications_enabled {
            if let Some(encrypted) = &user.slack_webhook_url {
                let already: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM notifications_sent
                     WHERE user_id = $1 AND month = $2 AND kind = 'monthly_report' AND channel = 'slack')",
                )
                .bind(user.id)
                .bind(&current_month)
                .fetch_one(pool)
                .await?;

                if !already {
                    match decrypt(&config.encryption_key, encrypted) {
                        Ok(url) => {
                            let mut text = format!(
                                "📅 *Garantify* — récapitulatif mensuel ({})\n",
                                current_month
                            );
                            if !expiring_s.is_empty() {
                                text.push_str("\n⚠️ *Expirent dans les 30 jours :*\n");
                                for eq in &expiring_s {
                                    text.push_str(&format!("• {} — {}\n", eq.name, eq.warranty_end_date));
                                }
                            }
                            if !expired_s.is_empty() {
                                text.push_str("\n❌ *Expirées le mois dernier :*\n");
                                for eq in &expired_s {
                                    text.push_str(&format!("• {} — {}\n", eq.name, eq.warranty_end_date));
                                }
                            }
                            match send_slack_message(&url, &text).await {
                                Ok(()) => {
                                    sqlx::query(
                                        "INSERT INTO notifications_sent (user_id, kind, month, channel)
                                         VALUES ($1, 'monthly_report', $2, 'slack') ON CONFLICT DO NOTHING",
                                    )
                                    .bind(user.id)
                                    .bind(&current_month)
                                    .execute(pool)
                                    .await?;
                                }
                                Err(e) => error!("Rapport Slack {} : {}", user.email, e),
                            }
                        }
                        Err(e) => error!("Déchiffrement webhook Slack (rapport) : {}", e),
                    }
                }
            }
        }
    }

    Ok(())
}

fn smtp_configured(config: &Config) -> bool {
    config.smtp_host.is_some() && config.smtp_username.is_some() && config.smtp_password.is_some()
}

impl From<EmailError> for JobError {
    fn from(e: EmailError) -> Self {
        JobError::Scheduler(e.to_string())
    }
}
