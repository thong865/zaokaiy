//! Customer side: my coupons and points, and price previews for the cart and the chat-order page.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use super::{engine, CartPromo, PromoReq, ShopSale};
use crate::{
    auth::{optional_user, AuthUser},
    error::{AppError, AppResult},
    AppState,
};

#[derive(Debug, Serialize, FromRow)]
pub struct CouponView {
    pub id: Uuid,
    pub code: String,
    pub campaign: String,
    pub description: String,
    pub shop_id: Option<Uuid>,
    pub shop_name: Option<String>,
    pub shop_slug: Option<String>,
    pub reward_type: String,
    pub reward_value: i64,
    pub max_discount_cents: i64,
    pub min_spend_cents: i64,
    pub currency: String,
    pub channels: Vec<String>,
    pub status: String,
    pub expires_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

/// Coupons matching `cond` ($1 = phone, $2 = shop id / user id depending on the caller).
pub async fn coupons_where(conn: &mut PgConnection, cond: &str, phone: Option<&str>, id: Option<Uuid>) -> AppResult<Vec<CouponView>> {
    Ok(sqlx::query_as(&format!(
        "SELECT c.id, c.code, p.name AS campaign, p.description, c.shop_id, s.name AS shop_name, s.slug AS shop_slug, c.reward_type,
                c.reward_value, c.max_discount_cents, c.min_spend_cents, c.currency, c.channels,
                CASE WHEN c.status = 'active' AND c.expires_at <= now() THEN 'expired' ELSE c.status END AS status, c.expires_at, c.used_at
         FROM promo_coupons c JOIN promo_campaigns p ON p.id = c.campaign_id
         LEFT JOIN shops s ON s.id = c.shop_id LEFT JOIN users u ON u.id = c.user_id
         WHERE {cond} ORDER BY (c.status = 'active' AND c.expires_at > now()) DESC, c.expires_at LIMIT 200"
    ))
    .bind(phone)
    .bind(id)
    .fetch_all(conn)
    .await?)
}

pub async fn my_rewards(State(st): State<AppState>, user: AuthUser) -> AppResult<Json<Value>> {
    let mut conn = st.db.acquire().await?;
    let phone: Option<String> = sqlx::query_scalar("SELECT phone FROM users WHERE id = $1").bind(user.id).fetch_one(&mut *conn).await?;
    let coupons = coupons_where(&mut conn, "(c.user_id = $2 OR ($1::text IS NOT NULL AND c.phone = $1)) AND p.trigger <> 'code'", phone.as_deref(), Some(user.id)).await?;
    let points: Vec<(Uuid, String, String, Option<String>, String, i64, i64, bool)> = sqlx::query_as(
        "SELECT s.id, s.name, s.slug, s.logo_url, s.currency, m.balance, COALESCE(l.point_value_cents, 0), COALESCE(l.enabled, false)
         FROM loyalty_members m JOIN shops s ON s.id = m.shop_id LEFT JOIN loyalty_settings l ON l.shop_id = m.shop_id
         WHERE $1::text IS NOT NULL AND m.phone = $1 ORDER BY m.balance DESC",
    )
    .bind(&phone)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Json(json!({
        "phone": phone,
        "coupons": coupons,
        "points": points.into_iter().map(|(id, name, slug, logo, cur, bal, val, on)| json!({
            "shop_id": id, "shop_name": name, "shop_slug": slug, "logo_url": logo, "currency": cur,
            "balance": bal, "value_cents": bal * val, "enabled": on
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct CartShop {
    pub shop_id: Uuid,
    pub subtotal_cents: i64,
    #[serde(default)]
    pub shipping_cents: i64,
}

#[derive(Deserialize)]
pub struct CartQuoteReq {
    #[serde(flatten)]
    pub promo: CartPromo,
    pub shops: Vec<CartShop>,
    pub phone: Option<String>,
}

/// Cart preview: what the code/points would take off each supplier order.
pub async fn cart_quote(State(st): State<AppState>, user: AuthUser, Json(r): Json<CartQuoteReq>) -> AppResult<Json<Value>> {
    if r.shops.is_empty() || r.shops.len() > 50 {
        return Err(AppError::bad("cart is empty"));
    }
    let mut conn = st.db.acquire().await?;
    let ids: Vec<Uuid> = r.shops.iter().map(|s| s.shop_id).collect();
    let cur: Vec<(Uuid, String)> = sqlx::query_as("SELECT id, currency FROM shops WHERE id = ANY($1)").bind(&ids).fetch_all(&mut *conn).await?;
    let sales: Vec<ShopSale> = r
        .shops
        .iter()
        .filter_map(|s| {
            cur.iter().find(|c| c.0 == s.shop_id).map(|c| ShopSale {
                shop_id: s.shop_id,
                currency: c.1.clone(),
                subtotal_cents: s.subtotal_cents.max(0),
                shipping_cents: s.shipping_cents.max(0),
            })
        })
        .collect();
    let (phone, trusted) = super::web_buyer(&mut conn, user.id, r.phone.as_deref()).await?;
    let applied = super::allocate(&mut conn, Some(user.id), phone.clone(), trusted, &r.promo, &sales, false).await?;
    // Points balances for every shop in the cart, so the page can offer them.
    let balances: Vec<(Uuid, i64, i64, i64, bool)> = sqlx::query_as(
        "SELECT m.shop_id, m.balance, l.point_value_cents, l.min_redeem_points, 'web' = ANY(l.channels)
         FROM loyalty_members m JOIN loyalty_settings l ON l.shop_id = m.shop_id AND l.enabled
         WHERE m.phone = $1 AND m.shop_id = ANY($2) AND $3",
    )
    .bind(&phone)
    .bind(&ids)
    .bind(trusted)
    .fetch_all(&mut *conn)
    .await?;
    Ok(Json(json!({
        "applied": applied.into_values().map(|(_, a)| a).collect::<Vec<_>>(),
        "points": balances.into_iter().map(|(shop, bal, val, min, web)| json!({
            "shop_id": shop, "balance": bal, "point_value_cents": val, "min_redeem_points": min, "usable": web
        })).collect::<Vec<_>>(),
        "points_need_verified_phone": !trusted,
    })))
}

#[derive(Deserialize)]
pub struct SocialQuoteReq {
    #[serde(flatten)]
    pub promo: PromoReq,
    pub phone: Option<String>,
}

/// Context for a chat order: the order's shop and amounts, the customer's phone, and whether a
/// signed-in customer may spend that phone's points.
pub async fn social_ctx(conn: &mut PgConnection, order_id: Uuid, user: Option<AuthUser>, phone: Option<&str>) -> AppResult<engine::Ctx> {
    let (shop_id, currency, subtotal, shipping, fee_payer, ship_phone): (Uuid, String, i64, i64, String, String) = sqlx::query_as(
        "SELECT shop_id, currency, subtotal_cents, shipping_cents, fee_payer, ship_phone FROM social_orders WHERE id = $1",
    )
    .bind(order_id)
    .fetch_one(&mut *conn)
    .await?;
    let phone = phone.filter(|p| !p.trim().is_empty()).unwrap_or(&ship_phone);
    let phone = super::normalize_phone(phone);
    let verified: Option<String> = match &user {
        Some(u) => sqlx::query_scalar("SELECT phone FROM users WHERE id = $1").bind(u.id).fetch_optional(&mut *conn).await?.flatten(),
        None => None,
    };
    Ok(engine::Ctx {
        channel: "social",
        shop_id,
        currency,
        user_id: user.map(|u| u.id),
        points_trusted: verified.is_some() && verified == phone,
        phone,
        subtotal_cents: subtotal,
        shipping_cents: if fee_payer == "buyer" { shipping } else { 0 },
    })
}

pub async fn social_quote(State(st): State<AppState>, headers: HeaderMap, Path(token): Path<String>, Json(r): Json<SocialQuoteReq>) -> AppResult<Json<Value>> {
    let order_id: Uuid = sqlx::query_scalar("SELECT id FROM social_orders WHERE token = $1 AND length($1) >= 20")
        .bind(&token)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let user = optional_user(&st, &headers);
    // Preview inside a transaction that is rolled back: the order's own coupon/points are given back
    // first, so re-confirming an order with the same code is priced correctly.
    let mut tx = st.db.begin().await?;
    let current: Option<(Option<String>, i64)> = sqlx::query_as(
        "SELECT CASE WHEN p.trigger = 'code' THEN p.code ELSE k.code END, r.points
         FROM promo_redemptions r LEFT JOIN promo_coupons k ON k.id = r.coupon_id LEFT JOIN promo_campaigns p ON p.id = k.campaign_id
         WHERE r.ref_type = 'social' AND r.ref_id = $1 AND r.reversed_at IS NULL",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;
    engine::reverse(&mut tx, "social", order_id, None).await?;
    let ctx = social_ctx(&mut tx, order_id, user, r.phone.as_deref()).await?;
    let a = engine::apply(&mut tx, &ctx, &r.promo, false).await?;
    let s = engine::settings(&mut tx, ctx.shop_id).await?;
    tx.rollback().await?;
    Ok(Json(json!({
        "applied": a,
        "points_enabled": s.enabled && s.channels.iter().any(|c| c == "social"),
        "point_value_cents": s.point_value_cents,
        "min_redeem_points": s.min_redeem_points,
        "points_trusted": ctx.points_trusted,
        "current": current.map(|(code, points)| json!({ "code": code, "points": points })),
    })))
}
