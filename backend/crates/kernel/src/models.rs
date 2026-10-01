use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;


#[derive(Debug, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>,
    #[serde(skip)]
    pub password_hash: Option<String>,
    pub display_name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
    /// Set while two-factor authentication is on.
    pub totp_enabled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct Shop {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub logo_url: Option<String>,
    pub currency: String,
    pub kind: String,
    pub created_at: DateTime<Utc>,
    pub auto_approve: bool,
    pub legal_name: String,
    pub tax_id: String,
    pub branch: String,
    pub address: String,
    pub phone: String,
    pub vat_bps: i32,
    pub prices_include_vat: bool,
    pub receipt_prefix: String,
    pub invoice_prefix: String,
    pub receipt_footer: String,
    pub social_triggers: Vec<String>,
    pub social_require_trigger: bool,
    pub social_hold_hours: i32,
    pub social_max_qty: i32,
    pub social_shipping_cents: i64,
    pub social_reply_template: String,
    pub social_soldout_template: String,
    pub payment_instructions: String,
    /// 'general' or 'vehicle' (vehicle showroom storefront).
    pub vertical: String,
    /// 'individual' or 'business' (business shops go through corporate KYC).
    pub entity_type: String,
    pub kyb_status: String,
    pub kyb_verified_at: Option<DateTime<Utc>>,
}


#[derive(Debug, Deserialize)]
pub struct Paging {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl Paging {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(24).clamp(1, 100)
    }
    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}
