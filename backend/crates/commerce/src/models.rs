//! Commerce models (shared ones — User, Shop, Paging — live in the kernel).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;


pub use kernel::models::*;

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
    /// First gallery image with responsive renditions: {url, thumb_url, width, height, variants, placeholder, color, alt}.
    pub cover: Option<serde_json::Value>,
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
    pub cover: Option<serde_json::Value>,
    /// The supplier shop passed business verification.
    pub shop_verified: bool,
    /// Card summaries from listing extensions, flattened into the product JSON
    /// (e.g. `"vehicle": {...}` when the vehicle core is enabled).
    #[serde(flatten)]
    pub ext: sqlx::types::Json<serde_json::Map<String, serde_json::Value>>,
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
    pub carrier_code: Option<String>,
    pub delivery_type: String,
    pub fee_payer: String,
    pub shipping_fee_cents: i64,
    pub payment_method: String,
    pub cod_fee_cents: i64,
    pub cod_amount_cents: i64,
    pub cod_status: String,
    pub cod_remit_ref: String,
    pub cod_collected_at: Option<DateTime<Utc>>,
    pub cod_remitted_at: Option<DateTime<Utc>>,
    pub tracking_no: String,
    pub shipped_at: Option<DateTime<Utc>>,
    pub grand_total_cents: i64,
    /// Coupon + points taken off the goods (module `promo`).
    pub discount_cents: i64,
    pub platform_discount_cents: i64,
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
