use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip)]
    pub password_hash: String,
    pub display_name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
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
}

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct Product {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: String,
    pub price_cents: i64,
    pub images: serde_json::Value,
    pub category: String,
    pub status: String,
    pub stock: i32,
    pub low_stock_threshold: i32,
    pub allow_resell: bool,
    pub commission_bps: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub category_id: Option<Uuid>,
    pub shop_category_id: Option<Uuid>,
    pub review_status: String,
    pub review_note: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub barcode: Option<String>,
    pub social_code: Option<String>,
}

/// Product joined with the supplier shop — used by public catalog views.
#[derive(Debug, Serialize, FromRow)]
pub struct CatalogProduct {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub sku: String,
    pub name: String,
    pub description: String,
    pub price_cents: i64,
    pub images: serde_json::Value,
    pub category: String,
    pub stock: i32,
    pub allow_resell: bool,
    pub commission_bps: i32,
    pub category_id: Option<Uuid>,
    pub shop_category_id: Option<Uuid>,
    pub shop_slug: String,
    pub shop_name: String,
    pub currency: String,
    /// Set when the product is shown through a reseller's storefront.
    pub via_shop_id: Option<Uuid>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct InventoryMovement {
    pub id: Uuid,
    pub product_id: Uuid,
    pub product_name: Option<String>,
    pub delta: i32,
    pub stock_after: i32,
    pub reason: String,
    pub ref_order_id: Option<Uuid>,
    pub note: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Partnership {
    pub id: Uuid,
    pub supplier_shop_id: Uuid,
    pub supplier_name: String,
    pub reseller_shop_id: Uuid,
    pub reseller_name: String,
    pub status: String,
    pub commission_bps: Option<i32>,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Order {
    pub id: Uuid,
    pub buyer_id: Uuid,
    pub status: String,
    pub total_cents: i64,
    pub currency: String,
    pub shipping_address: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct OrderItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Uuid,
    pub product_name: String,
    pub supplier_shop_id: Uuid,
    pub seller_shop_id: Option<Uuid>,
    pub qty: i32,
    pub unit_price_cents: i64,
    pub commission_bps: i32,
    pub commission_cents: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Commission {
    pub id: Uuid,
    pub order_item_id: Uuid,
    pub order_id: Uuid,
    pub product_name: String,
    pub beneficiary_shop_id: Uuid,
    pub supplier_shop_id: Uuid,
    pub supplier_name: String,
    pub amount_cents: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AdCampaign {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub product_id: Uuid,
    pub headline: String,
    pub body: String,
    pub image_url: Option<String>,
    pub budget_cents: i64,
    pub cpc_cents: i64,
    pub spent_cents: i64,
    pub impressions: i64,
    pub clicks: i64,
    pub status: String,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Content {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub product_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub media_url: Option<String>,
    pub ai_generated: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
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
