use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Equipment {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: String,
    pub purchase_type: String,
    pub product_url: Option<String>,
    pub image_path: Option<String>,
    pub invoice_path: Option<String>,
    pub purchase_date: NaiveDate,
    pub warranty_months: i32,
    pub warranty_end_date: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Equipment {
    /// Calcule le nombre de jours restants (négatif si expiré)
    pub fn days_left(&self) -> i64 {
        let today = chrono::Local::now().date_naive();
        (self.warranty_end_date - today).num_days()
    }

    /// "valid" | "expiring" | "expired" — utilisé comme data-status pour Alpine.js
    pub fn status_label(&self) -> &'static str {
        let d = self.days_left();
        if d < 0 {
            "expired"
        } else if d <= 30 {
            "expiring"
        } else {
            "valid"
        }
    }

    pub fn badge_class(&self) -> &'static str {
        match self.status_label() {
            "expired" => "g-badge g-badge-danger",
            "expiring" => "g-badge g-badge-warning",
            _ => "g-badge g-badge-ok",
        }
    }

    pub fn status_text(&self) -> String {
        let d = self.days_left();
        match d {
            d if d < 0 => format!("Expiré il y a {} j.", -d),
            0 => "Expire aujourd'hui".into(),
            d => format!("{} j. restants", d),
        }
    }

    pub fn category_label(&self) -> &'static str {
        match self.category.as_str() {
            "electromenager" => "Électroménager",
            "informatique" => "Informatique",
            "multimedia" => "Multimédia",
            _ => "Autre",
        }
    }
}

/// Calcule warranty_end_date à partir d'une date d'achat et d'une durée en mois.
pub fn compute_warranty_end(purchase_date: NaiveDate, warranty_months: i32) -> NaiveDate {
    use chrono::Months;
    purchase_date + Months::new(warranty_months as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn warranty_end_exact_12_months() {
        assert_eq!(
            compute_warranty_end(date(2024, 1, 15), 12),
            date(2025, 1, 15)
        );
    }

    #[test]
    fn warranty_end_24_months() {
        assert_eq!(compute_warranty_end(date(2023, 6, 1), 24), date(2025, 6, 1));
    }

    #[test]
    fn warranty_end_month_overflow_clips_to_last_day() {
        // Jan 31 + 1 mois = Feb 28 (chrono clamp au dernier jour du mois)
        let end = compute_warranty_end(date(2024, 1, 31), 1);
        assert_eq!(end, date(2024, 2, 29)); // 2024 est bissextile
    }

    #[test]
    fn warranty_end_1_month() {
        assert_eq!(
            compute_warranty_end(date(2024, 3, 15), 1),
            date(2024, 4, 15)
        );
    }
}
