//! # Promotions & loyalty points
//!
//! **Campaigns** are run by the platform (admin, `shop_id` NULL) or by a shop:
//! * `signup` (platform) — every new account that signed up with one of the chosen methods
//!   (Google, Facebook, WhatsApp, email) gets a coupon. A background job hands them out.
//! * `first_order` (shop) — after a customer's first completed order at the shop: a coupon for the
//!   next order, or bonus points.
//! * `code` (both) — anyone who types the code at checkout, once per customer (`LIVE10`).
//!
//! Rewards: an amount off, a percentage off (with a cap), free shipping, or bonus points.
//! Platform coupons are paid for by the platform (`platform_discount_cents`); the shop is owed that part.
//!
//! **Loyalty points** are per shop and keyed by the customer's phone number, the one thing web,
//! chat and counter customers all have. Completed sales earn points; points are spent at checkout,
//! on the chat-order page (signed in with the member phone) and at the POS (cashier looks up the phone).
//! Cancelling or voiding a sale gives the coupon and points back and takes back the points it earned.
//!
//! Switch off with `PROMO_ENABLED=false`: routes 404 and every hook below does nothing.

use std::{collections::BTreeMap, sync::LazyLock, time::Duration};

use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    AppState,
};

mod campaigns;
mod customer;
pub mod engine;
pub mod i18n;
mod loyalty;

pub use customer::social_ctx as customer_social_ctx;
pub use engine::{Applied, Ctx, PromoReq};

pub struct Config {
    pub enabled: bool,
}

static CFG: LazyLock<Config> =
    LazyLock::new(|| Config { enabled: std::env::var("PROMO_ENABLED").map(|v| v.trim().to_lowercase() != "false").unwrap_or(true) });

pub fn enabled() -> bool {
    CFG.enabled
}

pub fn public_status() -> Value {
    json!({ "enabled": enabled() })
}

/// E.164 phone (same rules as everywhere else on the platform).
pub fn normalize_phone(raw: &str) -> Option<String> {
    super::cod_risk::identity::normalize(raw)
}

pub fn router() -> kernel::Routes {
    kernel::Routes::new()
        // shop
        .route("/shops/{id}/promo/campaigns", get(campaigns::shop_list).post(campaigns::shop_create))
        .route("/promo/campaigns/{id}", axum::routing::patch(campaigns::shop_update))
        .route("/shops/{id}/loyalty", get(loyalty::get_settings).put(loyalty::put_settings))
        .route("/shops/{id}/loyalty/members", get(loyalty::members))
        .route("/shops/{id}/loyalty/members/{member}", get(loyalty::member))
        .route("/shops/{id}/loyalty/members/{member}/adjust", post(loyalty::adjust))
        .route("/shops/{id}/loyalty/lookup", get(loyalty::lookup))
        .route("/shops/{id}/loyalty/quote", post(loyalty::pos_quote))
        // admin
        .route("/admin/promo/campaigns", get(campaigns::admin_list).post(campaigns::admin_create))
        .route("/admin/promo/campaigns/{id}", axum::routing::patch(campaigns::admin_update))
        .route("/admin/promo/liability", get(campaigns::liability))
        // customers
        .route("/me/rewards", get(customer::my_rewards))
        .route("/promo/quote", post(customer::cart_quote))
        .route("/public/social-orders/{token}/promo", post(customer::social_quote))
}

pub fn spawn_jobs(st: AppState) {
    tracing::info!("promo module enabled");
    tokio::spawn(async move {
        // PROMO_JOB_SECS: how often sign-up coupons are handed out (default 60 s; tests use 1).
        let secs = std::env::var("PROMO_JOB_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(60u64).max(1);
        let mut tick = tokio::time::interval(Duration::from_secs(secs));
        let mut n: u64 = 0;
        loop {
            tick.tick().await;
            match campaigns::grant_signups(&st).await {
                Ok(0) => {}
                Ok(k) => tracing::info!(granted = k, "promo: sign-up coupons"),
                Err(e) => tracing::warn!(error = %e, "promo sign-up job failed"),
            }
            if n.is_multiple_of(3600 / secs.min(3600)) {
                if let Err(e) = loyalty::expire_points(&st).await {
                    tracing::warn!(error = %e, "promo points expiry failed");
                }
            }
            n += 1;
        }
    });
}

// ---------------------------------------------------------------------------
// Hooks called by the core (each a no-op when the module is off)
// ---------------------------------------------------------------------------

/// Web cart: coupon code (goes to one shop's order) + points per shop.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CartPromo {
    pub code: Option<String>,
    #[serde(default)]
    pub points: BTreeMap<Uuid, i64>,
}

impl CartPromo {
    pub fn is_empty(&self) -> bool {
        PromoReq { code: self.code.clone(), points: 0 }.code().is_none() && self.points.values().all(|p| *p <= 0)
    }
}

/// One supplier's part of a cart.
#[derive(Debug, Clone)]
pub struct ShopSale {
    pub shop_id: Uuid,
    pub currency: String,
    pub subtotal_cents: i64,
    pub shipping_cents: i64,
}

/// Who is buying on the web: the signed-in user; their verified phone (WhatsApp sign-in) is their member id.
pub async fn web_buyer(conn: &mut PgConnection, user_id: Uuid, shipping_phone: Option<&str>) -> AppResult<(Option<String>, bool)> {
    let verified: Option<String> = sqlx::query_scalar("SELECT phone FROM users WHERE id = $1").bind(user_id).fetch_optional(&mut *conn).await?.flatten();
    Ok(match verified {
        Some(p) => (Some(p), true),
        None => (shipping_phone.and_then(normalize_phone), false),
    })
}

/// Splits a cart promo over the supplier orders: the coupon goes to the largest order it is valid
/// for; points go to the shop they belong to.
pub async fn allocate(
    conn: &mut PgConnection,
    user_id: Option<Uuid>,
    phone: Option<String>,
    points_trusted: bool,
    req: &CartPromo,
    sales: &[ShopSale],
    lock: bool,
) -> AppResult<BTreeMap<Uuid, (Ctx, Applied)>> {
    let mut out = BTreeMap::new();
    if !enabled() || req.is_empty() {
        return Ok(out);
    }
    let ctx_for = |s: &ShopSale| Ctx {
        channel: "web",
        shop_id: s.shop_id,
        currency: s.currency.clone(),
        user_id,
        phone: phone.clone(),
        points_trusted,
        subtotal_cents: s.subtotal_cents,
        shipping_cents: s.shipping_cents,
    };
    let code = PromoReq { code: req.code.clone(), points: 0 }.code();
    let mut coupon_shop = None;
    if let Some(code) = &code {
        let mut order: Vec<&ShopSale> = sales.iter().collect();
        order.sort_by_key(|s| -s.subtotal_cents);
        let mut first_err: Option<AppError> = None;
        for s in order {
            let r = engine::apply(conn, &ctx_for(s), &PromoReq { code: Some(code.clone()), points: 0 }, false).await;
            match r {
                Ok(_) => {
                    coupon_shop = Some(s.shop_id);
                    break;
                }
                Err(e) => {
                    let other_shop = matches!(&e, AppError::BadRequest(m) if m == "this coupon is for another shop");
                    if first_err.is_none() || (!other_shop && matches!(&first_err, Some(AppError::BadRequest(m)) if m == "this coupon is for another shop")) {
                        first_err = Some(e);
                    }
                }
            }
        }
        if coupon_shop.is_none() {
            return Err(first_err.unwrap_or_else(|| AppError::bad("coupon code not found")));
        }
    }
    for s in sales {
        let points = req.points.get(&s.shop_id).copied().unwrap_or(0).max(0);
        let code = if coupon_shop == Some(s.shop_id) { code.clone() } else { None };
        if code.is_none() && points == 0 {
            continue;
        }
        let ctx = ctx_for(s);
        let a = engine::apply(conn, &ctx, &PromoReq { code, points }, lock).await?;
        out.insert(s.shop_id, (ctx, a));
    }
    Ok(out)
}

pub async fn apply(conn: &mut PgConnection, ctx: &Ctx, req: &PromoReq, lock: bool) -> AppResult<Applied> {
    if !enabled() || req.is_empty() {
        return Ok(Applied::default());
    }
    engine::apply(conn, ctx, req, lock).await
}

pub async fn commit(conn: &mut PgConnection, ctx: &Ctx, a: &Applied, ref_type: &str, ref_id: Uuid, by: Option<Uuid>) -> AppResult<()> {
    if !enabled() {
        return Ok(());
    }
    engine::commit(conn, ctx, a, ref_type, ref_id, by).await
}

/// A sale was cancelled, voided or refused.
pub async fn sale_cancelled(conn: &mut PgConnection, ref_type: &str, ref_id: Uuid, by: Option<Uuid>) -> AppResult<()> {
    if !enabled() {
        return Ok(());
    }
    engine::reverse(conn, ref_type, ref_id, by).await
}

/// Marketplace order completed: points for the buyer (verified phone, else the shipping phone).
pub async fn order_completed(conn: &mut PgConnection, order_id: Uuid) -> AppResult<()> {
    if !enabled() {
        return Ok(());
    }
    let row: Option<(Uuid, String, String, i64)> = sqlx::query_as(
        "SELECT (SELECT supplier_shop_id FROM order_items WHERE order_id = o.id LIMIT 1),
                COALESCE(u.phone, o.shipping_address->>'phone', ''),
                COALESCE(o.shipping_address->>'name', u.display_name, ''),
                o.total_cents - o.discount_cents
         FROM orders o JOIN users u ON u.id = o.buyer_id WHERE o.id = $1",
    )
    .bind(order_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((shop, phone, name, net)) = row {
        engine::earn(conn, shop, "web", &phone, &name, net, "order", order_id).await?;
    }
    Ok(())
}

/// Chat order completed.
pub async fn social_completed(conn: &mut PgConnection, order_id: Uuid) -> AppResult<()> {
    if !enabled() {
        return Ok(());
    }
    let row: Option<(Uuid, String, String, i64)> = sqlx::query_as(
        "SELECT shop_id, ship_phone, ship_name, greatest(subtotal_cents - discount_cents, 0) FROM social_orders WHERE id = $1",
    )
    .bind(order_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((shop, phone, name, net)) = row {
        engine::earn(conn, shop, "social", &phone, &name, net, "social", order_id).await?;
    }
    Ok(())
}

/// POS sale: points for the member at the counter.
pub async fn pos_completed(conn: &mut PgConnection, shop_id: Uuid, phone: &str, name: &str, net_cents: i64, sale_id: Uuid) -> AppResult<Option<i64>> {
    if !enabled() || phone.trim().is_empty() {
        return Ok(None);
    }
    engine::earn(conn, shop_id, "pos", phone, name, net_cents, "pos", sale_id).await
}
