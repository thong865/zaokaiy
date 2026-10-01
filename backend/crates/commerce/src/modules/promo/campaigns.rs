//! Campaign management (platform admin + shops) and the sign-up grant job.

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use super::engine::{self, CHANNELS};
use crate::{
    auth::{AdminUser, AuthUser},
    error::{AppError, AppResult},
    routes::owned_shop,
    AppState,
};

pub const PROVIDERS: [&str; 4] = ["google", "facebook", "whatsapp", "email"];
const CURRENCIES: [&str; 3] = ["LAK", "THB", "USD"];

#[derive(Debug, Serialize, FromRow)]
pub struct Campaign {
    pub id: Uuid,
    pub shop_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub trigger: String,
    pub providers: Vec<String>,
    pub code: Option<String>,
    pub reward_type: String,
    pub reward_value: i64,
    pub max_discount_cents: i64,
    pub min_spend_cents: i64,
    pub currency: String,
    pub coupon_days: i32,
    pub max_grants: i32,
    pub grants: i32,
    pub channels: Vec<String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    /// Coupons used / discount given (not reversed).
    pub used: i64,
    pub discount_cents: i64,
}

const LIST: &str = "SELECT c.id, c.shop_id, c.name, c.description, c.trigger, c.providers, c.code, c.reward_type, c.reward_value,
        c.max_discount_cents, c.min_spend_cents, c.currency, c.coupon_days, c.max_grants, c.grants, c.channels, c.starts_at,
        c.ends_at, c.active, c.created_at,
        (SELECT count(*) FROM promo_coupons k WHERE k.campaign_id = c.id AND k.status = 'used') AS used,
        COALESCE((SELECT sum(r.coupon_cents) FROM promo_redemptions r JOIN promo_coupons k ON k.id = r.coupon_id
                  WHERE k.campaign_id = c.id AND r.reversed_at IS NULL), 0)::bigint AS discount_cents
    FROM promo_campaigns c";

#[derive(Debug, Deserialize)]
pub struct CampaignReq {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub trigger: String,
    #[serde(default)]
    pub providers: Vec<String>,
    pub code: Option<String>,
    pub reward_type: String,
    #[serde(default)]
    pub reward_value: i64,
    #[serde(default)]
    pub max_discount_cents: i64,
    #[serde(default)]
    pub min_spend_cents: i64,
    pub currency: Option<String>,
    pub coupon_days: Option<i32>,
    #[serde(default)]
    pub max_grants: i32,
    pub channels: Option<Vec<String>>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub active: Option<bool>,
}

/// Checked, normalised values ready to store.
struct Clean {
    name: String,
    description: String,
    trigger: String,
    providers: Vec<String>,
    code: Option<String>,
    reward_type: String,
    reward_value: i64,
    max_discount_cents: i64,
    min_spend_cents: i64,
    currency: String,
    coupon_days: i32,
    max_grants: i32,
    channels: Vec<String>,
    starts_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    active: bool,
}

fn validate(r: CampaignReq, platform: bool, shop_currency: &str) -> AppResult<Clean> {
    let name = r.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::bad("give the campaign a name (up to 120 characters)"));
    }
    let triggers: &[&str] = if platform { &["signup", "code"] } else { &["first_order", "code"] };
    if !triggers.contains(&r.trigger.as_str()) {
        return Err(AppError::bad(if platform { "trigger must be signup or code" } else { "trigger must be first_order or code" }));
    }
    let mut providers: Vec<String> = r.providers.iter().map(|p| p.trim().to_lowercase()).filter(|p| !p.is_empty()).collect();
    providers.sort();
    providers.dedup();
    if providers.iter().any(|p| !PROVIDERS.contains(&p.as_str())) {
        return Err(AppError::bad("sign-up methods must be google, facebook, whatsapp or email"));
    }
    if r.trigger != "signup" {
        providers.clear();
    }
    let code = match (r.trigger.as_str(), r.code.as_deref().map(|c| c.trim().to_uppercase())) {
        ("code", Some(c)) if (3..=20).contains(&c.len()) && c.chars().all(|x| x.is_ascii_alphanumeric()) => Some(c),
        ("code", _) => return Err(AppError::bad("the code must be 3–20 letters or digits")),
        _ => None,
    };
    let kinds: &[&str] = if platform { &["amount", "percent", "free_shipping"] } else { &["amount", "percent", "free_shipping", "points"] };
    if !kinds.contains(&r.reward_type.as_str()) {
        return Err(AppError::bad(if platform { "reward must be amount, percent or free_shipping" } else { "reward must be amount, percent, free_shipping or points" }));
    }
    if r.reward_type == "points" && r.trigger != "first_order" {
        return Err(AppError::bad("bonus points are given with first-order campaigns"));
    }
    let value = match r.reward_type.as_str() {
        "free_shipping" => 0,
        "percent" if !(1..=10_000).contains(&r.reward_value) => return Err(AppError::bad("percent must be between 0.01% and 100%")),
        _ if r.reward_value <= 0 && r.reward_type != "percent" => return Err(AppError::bad("enter the reward value")),
        _ => r.reward_value,
    };
    if r.max_discount_cents < 0 || r.min_spend_cents < 0 || r.max_grants < 0 {
        return Err(AppError::bad("amounts can't be negative"));
    }
    let currency = if platform { r.currency.unwrap_or_else(|| "LAK".into()).to_uppercase() } else { shop_currency.to_string() };
    if !CURRENCIES.contains(&currency.as_str()) {
        return Err(AppError::bad("currency must be LAK, THB or USD"));
    }
    let coupon_days = r.coupon_days.unwrap_or(30);
    if !(1..=3650).contains(&coupon_days) {
        return Err(AppError::bad("coupons must be valid for 1 to 3650 days"));
    }
    let mut channels = r.channels.unwrap_or_else(|| CHANNELS.map(String::from).to_vec());
    channels.sort();
    channels.dedup();
    if channels.is_empty() || channels.iter().any(|c| !CHANNELS.contains(&c.as_str())) {
        return Err(AppError::bad("choose where it can be used: web, social or pos"));
    }
    let starts_at = r.starts_at.unwrap_or_else(Utc::now);
    if r.ends_at.is_some_and(|e| e <= starts_at) {
        return Err(AppError::bad("the end date must be after the start"));
    }
    Ok(Clean {
        name,
        description: r.description.trim().chars().take(1000).collect(),
        trigger: r.trigger,
        providers,
        code,
        reward_type: r.reward_type,
        reward_value: value,
        max_discount_cents: r.max_discount_cents,
        min_spend_cents: r.min_spend_cents,
        currency,
        coupon_days,
        max_grants: r.max_grants,
        channels,
        starts_at,
        ends_at: r.ends_at,
        active: r.active.unwrap_or(true),
    })
}

async fn load(st: &AppState, id: Uuid) -> AppResult<Campaign> {
    sqlx::query_as(&format!("{LIST} WHERE c.id = $1")).bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)
}

async fn save(st: &AppState, id: Option<Uuid>, shop_id: Option<Uuid>, c: Clean, by: Uuid) -> AppResult<Uuid> {
    let q = match id {
        None => sqlx::query_scalar(
            "INSERT INTO promo_campaigns (name, description, trigger, providers, code, reward_type, reward_value, max_discount_cents,
                 min_spend_cents, currency, coupon_days, max_grants, channels, starts_at, ends_at, active, shop_id, created_by)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18) RETURNING id",
        ),
        Some(_) => sqlx::query_scalar(
            "UPDATE promo_campaigns SET name=$1, description=$2, trigger=$3, providers=$4, code=$5, reward_type=$6, reward_value=$7,
                 max_discount_cents=$8, min_spend_cents=$9, currency=$10, coupon_days=$11, max_grants=$12, channels=$13, starts_at=$14,
                 ends_at=$15, active=$16, updated_at=now()
             WHERE id=$19 AND shop_id IS NOT DISTINCT FROM $17 AND $18::uuid IS NOT NULL RETURNING id",
        ),
    };
    let r: Option<Uuid> = q
        .bind(c.name)
        .bind(c.description)
        .bind(c.trigger)
        .bind(c.providers)
        .bind(c.code)
        .bind(c.reward_type)
        .bind(c.reward_value)
        .bind(c.max_discount_cents)
        .bind(c.min_spend_cents)
        .bind(c.currency)
        .bind(c.coupon_days)
        .bind(c.max_grants)
        .bind(c.channels)
        .bind(c.starts_at)
        .bind(c.ends_at)
        .bind(c.active)
        .bind(shop_id)
        .bind(by)
        .bind(id)
        .fetch_optional(&st.db)
        .await
        .map_err(|e| match AppError::from(e) {
            AppError::Conflict(_) => AppError::Conflict("another campaign already uses this code".into()),
            other => other,
        })?;
    r.ok_or(AppError::NotFound)
}

// --- shop ---

pub async fn shop_list(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Vec<Campaign>>> {
    owned_shop(&st, shop_id, &user).await?;
    Ok(Json(sqlx::query_as(&format!("{LIST} WHERE c.shop_id = $1 ORDER BY c.created_at DESC")).bind(shop_id).fetch_all(&st.db).await?))
}

pub async fn shop_create(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<CampaignReq>) -> AppResult<Json<Campaign>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    let c = validate(r, false, &shop.currency)?;
    let id = save(&st, None, Some(shop_id), c, user.id).await?;
    Ok(Json(load(&st, id).await?))
}

pub async fn shop_update(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<CampaignReq>) -> AppResult<Json<Campaign>> {
    let shop_id: Option<Uuid> = sqlx::query_scalar("SELECT shop_id FROM promo_campaigns WHERE id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let shop_id = shop_id.ok_or(AppError::Forbidden)?;
    let shop = owned_shop(&st, shop_id, &user).await?;
    let c = validate(r, false, &shop.currency)?;
    save(&st, Some(id), Some(shop_id), c, user.id).await?;
    Ok(Json(load(&st, id).await?))
}

// --- admin (platform campaigns) ---

pub async fn admin_list(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Vec<Campaign>>> {
    Ok(Json(sqlx::query_as(&format!("{LIST} WHERE c.shop_id IS NULL ORDER BY c.created_at DESC")).fetch_all(&st.db).await?))
}

pub async fn admin_create(State(st): State<AppState>, a: AdminUser, Json(r): Json<CampaignReq>) -> AppResult<Json<Campaign>> {
    let c = validate(r, true, "")?;
    let id = save(&st, None, None, c, a.0.id).await?;
    Ok(Json(load(&st, id).await?))
}

pub async fn admin_update(State(st): State<AppState>, a: AdminUser, Path(id): Path<Uuid>, Json(r): Json<CampaignReq>) -> AppResult<Json<Campaign>> {
    let c = validate(r, true, "")?;
    save(&st, Some(id), None, c, a.0.id).await?;
    Ok(Json(load(&st, id).await?))
}

/// What the platform owes each shop for platform-funded discounts.
pub async fn liability(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Value>> {
    let rows: Vec<(Uuid, String, String, i64, i64)> = sqlx::query_as(
        "SELECT r.shop_id, s.name, r.currency, sum(r.platform_cents)::bigint, count(*)
         FROM promo_redemptions r JOIN shops s ON s.id = r.shop_id
         WHERE r.reversed_at IS NULL AND r.platform_cents > 0
         GROUP BY 1, 2, 3 ORDER BY 4 DESC",
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!(rows
        .into_iter()
        .map(|(id, name, cur, cents, n)| json!({ "shop_id": id, "shop_name": name, "currency": cur, "amount_cents": cents, "sales": n }))
        .collect::<Vec<_>>())))
}

// --- sign-up job ---

/// Gives every eligible new account its sign-up coupon. The sign-up method is the login method
/// created with the account (within 5 minutes), or `email` for password accounts.
pub async fn grant_signups(st: &AppState) -> AppResult<u64> {
    let due: Vec<(Uuid, Uuid, Option<String>)> = sqlx::query_as(
        "WITH m AS (
            SELECT u.id, u.phone, u.created_at,
                   COALESCE((SELECT i.provider FROM user_identities i WHERE i.user_id = u.id AND i.created_at <= u.created_at + interval '5 minutes'
                             ORDER BY i.created_at LIMIT 1), 'email') AS method
            FROM users u WHERE u.created_at > now() - interval '400 days')
         SELECT c.id, m.id, m.phone FROM promo_campaigns c JOIN m ON m.created_at >= c.starts_at AND (c.ends_at IS NULL OR m.created_at < c.ends_at)
         WHERE c.shop_id IS NULL AND c.trigger = 'signup' AND c.active AND (c.ends_at IS NULL OR c.ends_at > now() - interval '1 day')
           AND (cardinality(c.providers) = 0 OR m.method = ANY(c.providers))
           AND (c.max_grants = 0 OR c.grants < c.max_grants)
           AND NOT EXISTS (SELECT 1 FROM promo_coupons k WHERE k.campaign_id = c.id AND k.user_id = m.id)
         ORDER BY m.created_at LIMIT 500",
    )
    .fetch_all(&st.db)
    .await?;
    let mut n = 0;
    for (camp, user, phone) in due {
        let mut tx = st.db.begin().await?;
        if engine::grant_coupon(&mut tx, camp, Some(user), phone.as_deref()).await?.is_some() {
            n += 1;
        }
        tx.commit().await?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(trigger: &str, reward: &str, value: i64) -> CampaignReq {
        CampaignReq {
            name: "Welcome".into(), description: String::new(), trigger: trigger.into(), providers: vec!["Facebook".into()],
            code: Some("live10".into()), reward_type: reward.into(), reward_value: value, max_discount_cents: 0, min_spend_cents: 0,
            currency: None, coupon_days: None, max_grants: 0, channels: None, starts_at: None, ends_at: None, active: None,
        }
    }

    #[test]
    fn rules() {
        let c = validate(req("signup", "amount", 2_000_000), true, "").unwrap();
        assert_eq!(c.providers, vec!["facebook"]);
        assert!(c.code.is_none());
        assert_eq!(c.currency, "LAK");
        assert!(validate(req("signup", "points", 10), true, "").is_err());
        assert!(validate(req("first_order", "amount", 10), true, "").is_err());
        assert!(validate(req("signup", "amount", 10), false, "THB").is_err());
        assert_eq!(validate(req("code", "percent", 1000), false, "THB").unwrap().code.as_deref(), Some("LIVE10"));
        assert!(validate(req("code", "points", 10), false, "THB").is_err());
        assert!(validate(req("first_order", "points", 50), false, "THB").is_ok());
        assert!(validate(req("code", "percent", 20_000), false, "THB").is_err());
    }
}
