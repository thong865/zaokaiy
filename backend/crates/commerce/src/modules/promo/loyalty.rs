//! Shop loyalty programme: settings, members, points ledger, counter lookup, expiry job.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use super::engine::{self, CHANNELS};
use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    routes::owned_shop,
    AppState,
};

fn settings_json(s: &engine::Settings) -> Value {
    json!({
        "enabled": s.enabled, "earn_per_cents": s.earn_per_cents, "point_value_cents": s.point_value_cents,
        "min_redeem_points": s.min_redeem_points, "max_redeem_bps": s.max_redeem_bps, "expiry_days": s.expiry_days, "channels": s.channels,
    })
}

pub async fn get_settings(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let mut conn = st.db.acquire().await?;
    let s = engine::settings(&mut conn, shop_id).await?;
    let (members, points): (i64, i64) = sqlx::query_as("SELECT count(*), COALESCE(sum(balance), 0)::bigint FROM loyalty_members WHERE shop_id = $1")
        .bind(shop_id)
        .fetch_one(&mut *conn)
        .await?;
    let mut v = settings_json(&s);
    v["stats"] = json!({ "members": members, "outstanding_points": points, "outstanding_cents": points * s.point_value_cents });
    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct SettingsReq {
    pub enabled: bool,
    pub earn_per_cents: i64,
    pub point_value_cents: i64,
    pub min_redeem_points: i64,
    pub max_redeem_bps: i32,
    #[serde(default)]
    pub expiry_days: i32,
    pub channels: Option<Vec<String>>,
}

pub async fn put_settings(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<SettingsReq>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    if r.earn_per_cents <= 0 || r.point_value_cents <= 0 {
        return Err(AppError::bad("earning and point value must be above zero"));
    }
    if r.min_redeem_points < 1 || !(1..=10_000).contains(&r.max_redeem_bps) || !(0..=3650).contains(&r.expiry_days) {
        return Err(AppError::bad("check the redeem rules: minimum 1 point, up to 100% of a bill, expiry 0–3650 days"));
    }
    let mut channels = r.channels.unwrap_or_else(|| CHANNELS.map(String::from).to_vec());
    channels.sort();
    channels.dedup();
    if channels.is_empty() || channels.iter().any(|c| !CHANNELS.contains(&c.as_str())) {
        return Err(AppError::bad("choose where it can be used: web, social or pos"));
    }
    sqlx::query(
        "INSERT INTO loyalty_settings (shop_id, enabled, earn_per_cents, point_value_cents, min_redeem_points, max_redeem_bps, expiry_days, channels)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
         ON CONFLICT (shop_id) DO UPDATE SET enabled=$2, earn_per_cents=$3, point_value_cents=$4, min_redeem_points=$5,
             max_redeem_bps=$6, expiry_days=$7, channels=$8, updated_at=now()",
    )
    .bind(shop_id)
    .bind(r.enabled)
    .bind(r.earn_per_cents)
    .bind(r.point_value_cents)
    .bind(r.min_redeem_points)
    .bind(r.max_redeem_bps)
    .bind(r.expiry_days)
    .bind(channels)
    .execute(&st.db)
    .await?;
    get_settings(State(st), user, Path(shop_id)).await
}

#[derive(Debug, Serialize, FromRow)]
pub struct Member {
    pub id: Uuid,
    pub phone: String,
    pub name: String,
    pub balance: i64,
    pub lifetime_points: i64,
    pub orders: i32,
    pub last_activity_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

const MEMBER_COLS: &str = "id, phone, name, balance, lifetime_points, orders, last_activity_at, created_at";

#[derive(Deserialize)]
pub struct MembersQ {
    pub q: Option<String>,
}

pub async fn members(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Query(q): Query<MembersQ>) -> AppResult<Json<Vec<Member>>> {
    owned_shop(&st, shop_id, &user).await?;
    let term = q.q.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let digits = term.as_deref().map(|t| t.chars().filter(char::is_ascii_digit).collect::<String>()).filter(|d| d.len() >= 3);
    Ok(Json(
        sqlx::query_as(&format!(
            "SELECT {MEMBER_COLS} FROM loyalty_members WHERE shop_id = $1
               AND ($2::text IS NULL OR name ILIKE '%' || $2 || '%' OR ($3::text IS NOT NULL AND phone LIKE '%' || $3 || '%'))
             ORDER BY last_activity_at DESC LIMIT 200"
        ))
        .bind(shop_id)
        .bind(term)
        .bind(digits)
        .fetch_all(&st.db)
        .await?,
    ))
}

async fn shop_member(st: &AppState, shop_id: Uuid, member: Uuid) -> AppResult<Member> {
    sqlx::query_as(&format!("SELECT {MEMBER_COLS} FROM loyalty_members WHERE id = $1 AND shop_id = $2"))
        .bind(member)
        .bind(shop_id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn member(State(st): State<AppState>, user: AuthUser, Path((shop_id, member)): Path<(Uuid, Uuid)>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let m = shop_member(&st, shop_id, member).await?;
    let ledger: Vec<(i64, i64, i64, String, Option<String>, Option<Uuid>, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, delta, balance_after, reason, ref_type, ref_id, note, created_at FROM loyalty_ledger WHERE member_id = $1 ORDER BY id DESC LIMIT 200",
    )
    .bind(member)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({
        "member": m,
        "ledger": ledger.into_iter().map(|(id, d, b, r, rt, ri, n, at)| json!({
            "id": id, "delta": d, "balance_after": b, "reason": r, "ref_type": rt, "ref_id": ri, "note": n, "created_at": at
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
pub struct AdjustReq {
    pub delta: i64,
    pub note: String,
}

pub async fn adjust(State(st): State<AppState>, user: AuthUser, Path((shop_id, member)): Path<(Uuid, Uuid)>, Json(r): Json<AdjustReq>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    shop_member(&st, shop_id, member).await?;
    if r.delta == 0 || r.note.trim().is_empty() {
        return Err(AppError::bad("enter the points to add or remove and a note"));
    }
    let mut tx = st.db.begin().await?;
    let bal: Option<i64> = sqlx::query_scalar(
        "UPDATE loyalty_members SET balance = balance + $2, lifetime_points = lifetime_points + greatest($2, 0), last_activity_at = now()
         WHERE id = $1 AND balance + $2 >= 0 RETURNING balance",
    )
    .bind(member)
    .bind(r.delta)
    .fetch_optional(&mut *tx)
    .await?;
    let bal = bal.ok_or_else(|| AppError::bad("the balance can't go below zero"))?;
    engine::ledger(&mut tx, member, r.delta, bal, "adjust", None, r.note.trim(), Some(user.id)).await?;
    tx.commit().await?;
    self::member(State(st), user, Path((shop_id, member))).await
}

#[derive(Deserialize)]
pub struct LookupQ {
    pub phone: String,
}

/// Counter: a customer's points and the coupons they can use at this shop.
pub async fn lookup(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Query(q): Query<LookupQ>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let phone = super::normalize_phone(&q.phone).ok_or_else(|| AppError::bad("enter a valid phone number"))?;
    let mut conn = st.db.acquire().await?;
    let s = engine::settings(&mut conn, shop_id).await?;
    let m: Option<Member> = sqlx::query_as(&format!("SELECT {MEMBER_COLS} FROM loyalty_members WHERE shop_id = $1 AND phone = $2"))
        .bind(shop_id)
        .bind(&phone)
        .fetch_optional(&mut *conn)
        .await?;
    let coupons = super::customer::coupons_where(&mut conn, "(c.phone = $1 OR u.phone = $1) AND (c.shop_id IS NULL OR c.shop_id = $2) AND 'pos' = ANY(c.channels) AND c.status = 'active' AND c.expires_at > now()", Some(&phone), Some(shop_id)).await?;
    Ok(Json(json!({
        "phone": phone,
        "member": m,
        "settings": settings_json(&s),
        "coupons": coupons,
    })))
}

/// Points of members inactive for longer than the shop's `expiry_days` expire.
pub async fn expire_points(st: &AppState) -> AppResult<u64> {
    let due: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT m.id, m.balance FROM loyalty_members m JOIN loyalty_settings s ON s.shop_id = m.shop_id
         WHERE s.expiry_days > 0 AND m.balance > 0 AND m.last_activity_at < now() - make_interval(days => s.expiry_days) LIMIT 1000",
    )
    .fetch_all(&st.db)
    .await?;
    let mut n = 0;
    for (id, _) in due {
        let mut tx = st.db.begin().await?;
        let bal: Option<i64> = sqlx::query_scalar("SELECT balance FROM loyalty_members WHERE id = $1 AND balance > 0 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(b) = bal {
            sqlx::query("UPDATE loyalty_members SET balance = 0 WHERE id = $1").bind(id).execute(&mut *tx).await?;
            engine::ledger(&mut tx, id, -b, 0, "expire", None, "inactive", None).await?;
            n += 1;
        }
        tx.commit().await?;
    }
    Ok(n)
}

#[derive(Deserialize)]
pub struct PosQuoteReq {
    #[serde(default)]
    pub member_phone: String,
    #[serde(flatten)]
    pub promo: super::PromoReq,
    /// Bill after line and bill discounts.
    pub subtotal_cents: i64,
}

/// Counter preview of what the member's coupon / points take off the bill.
pub async fn pos_quote(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<PosQuoteReq>) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    let phone = super::normalize_phone(&r.member_phone);
    if !r.member_phone.trim().is_empty() && phone.is_none() {
        return Err(AppError::bad("enter a valid phone number"));
    }
    let ctx = engine::Ctx {
        channel: "pos",
        shop_id,
        currency: shop.currency.clone(),
        user_id: None,
        phone,
        points_trusted: true,
        subtotal_cents: r.subtotal_cents.max(0),
        shipping_cents: 0,
    };
    let mut conn = st.db.acquire().await?;
    Ok(Json(json!(engine::apply(&mut conn, &ctx, &r.promo, false).await?)))
}
