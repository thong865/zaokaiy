//! Pairing a terminal (device-authorisation flow) and managing a shop's terminals.

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use rand::{Rng, RngCore};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    routes::owned_shop,
    AppState,
};

use super::{device::TOKEN_PREFIX, sha256_hex};

const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const PAIR_TTL_MINUTES: i64 = 10;

fn new_code() -> String {
    let mut rng = rand::thread_rng();
    (0..8).map(|_| CODE_ALPHABET[rng.gen_range(0..CODE_ALPHABET.len())] as char).collect()
}

fn random_hex() -> String {
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw);
    hex::encode(raw)
}

/// "abcd-efgh" / "ABCD EFGH" → "ABCDEFGH".
fn normalize_code(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

fn pretty(code: &str) -> String {
    if code.len() == 8 {
        format!("{}-{}", &code[..4], &code[4..])
    } else {
        code.to_string()
    }
}

fn clip(s: &str, n: usize) -> String {
    s.trim().chars().take(n).collect()
}

#[derive(Deserialize)]
pub struct StartReq {
    pub name: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub app_version: String,
    pub install_id: String,
}

/// Terminal: ask for a pairing code. Public (the terminal has no credentials yet).
pub async fn start(State(st): State<AppState>, Json(req): Json<StartReq>) -> AppResult<Json<Value>> {
    let name = clip(&req.name, 80);
    if name.is_empty() {
        return Err(AppError::bad("give this terminal a name"));
    }
    let install = req.install_id.trim();
    if !(8..=100).contains(&install.len()) {
        return Err(AppError::bad("invalid terminal id"));
    }
    sqlx::query("DELETE FROM pos_pairings WHERE expires_at < now() - interval '1 day'").execute(&st.db).await?;
    let secret = random_hex();
    for _ in 0..5 {
        let code = new_code();
        let row: Option<(Uuid, DateTime<Utc>)> = sqlx::query_as(
            "INSERT INTO pos_pairings (code, poll_hash, name, platform, app_version, install_id, expires_at)
             VALUES ($1,$2,$3,$4,$5,$6, now() + make_interval(mins => $7::int))
             ON CONFLICT (code) DO NOTHING RETURNING id, expires_at",
        )
        .bind(&code)
        .bind(sha256_hex(&secret))
        .bind(&name)
        .bind(clip(&req.platform, 40))
        .bind(clip(&req.app_version, 40))
        .bind(install)
        .bind(PAIR_TTL_MINUTES as i32)
        .fetch_optional(&st.db)
        .await?;
        if let Some((id, expires_at)) = row {
            return Ok(Json(json!({
                "pair_id": id, "code": pretty(&code), "poll_secret": secret,
                "expires_at": expires_at, "interval": 3
            })));
        }
    }
    Err(AppError::Internal("could not allocate a pairing code".into()))
}

/// Web: what is being paired (for the approval page).
pub async fn info(State(st): State<AppState>, _user: AuthUser, Path(code): Path<String>) -> AppResult<Json<Value>> {
    type Row = (String, String, String, DateTime<Utc>, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<String>);
    let row: Option<Row> = sqlx::query_as(
        "SELECT p.code, p.name, p.platform, p.created_at, p.expires_at, p.approved_at, p.consumed_at, s.name
         FROM pos_pairings p LEFT JOIN shops s ON s.id = p.shop_id WHERE p.code = $1",
    )
    .bind(normalize_code(&code))
    .fetch_optional(&st.db)
    .await?;
    let (code, name, platform, created_at, expires_at, approved_at, consumed_at, shop) =
        row.ok_or_else(|| AppError::bad("this pairing code is invalid or has expired"))?;
    let status = if consumed_at.is_some() {
        "used"
    } else if approved_at.is_some() {
        "approved"
    } else if expires_at < Utc::now() {
        "expired"
    } else {
        "pending"
    };
    Ok(Json(json!({
        "code": pretty(&code), "name": name, "platform": platform, "created_at": created_at,
        "expires_at": expires_at, "status": status, "shop": shop
    })))
}

#[derive(Deserialize)]
pub struct ApproveReq {
    pub code: String,
    pub shop_id: Uuid,
}

/// Web: a signed-in owner or staff member with POS access approves the code for one shop.
pub async fn approve(State(st): State<AppState>, user: AuthUser, Json(req): Json<ApproveReq>) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, req.shop_id, &user).await?;
    let ok: Option<Uuid> = sqlx::query_scalar(
        "UPDATE pos_pairings SET approved_by = $2, shop_id = $3, approved_at = now()
         WHERE code = $1 AND approved_at IS NULL AND expires_at > now() RETURNING id",
    )
    .bind(normalize_code(&req.code))
    .bind(user.id)
    .bind(shop.id)
    .fetch_optional(&st.db)
    .await?;
    ok.ok_or_else(|| AppError::bad("this pairing code is invalid or has expired"))?;
    Ok(Json(json!({ "ok": true, "shop": { "id": shop.id, "name": shop.name } })))
}

#[derive(Deserialize)]
pub struct PollReq {
    pub pair_id: Uuid,
    pub poll_secret: String,
}

/// Terminal: pending | expired | approved (+ device token, returned exactly once).
pub async fn poll(State(st): State<AppState>, Json(req): Json<PollReq>) -> AppResult<Json<Value>> {
    let mut tx = st.db.begin().await?;
    type Row = (String, String, String, String, DateTime<Utc>, Option<Uuid>, Option<Uuid>, Option<DateTime<Utc>>);
    let row: Option<Row> = sqlx::query_as(
        "SELECT name, platform, app_version, install_id, expires_at, approved_by, shop_id, consumed_at
         FROM pos_pairings WHERE id = $1 AND poll_hash = $2 FOR UPDATE",
    )
    .bind(req.pair_id)
    .bind(sha256_hex(req.poll_secret.trim()))
    .fetch_optional(&mut *tx)
    .await?;
    let (name, platform, app_version, install_id, expires_at, approved_by, shop_id, consumed_at) =
        row.ok_or_else(|| AppError::bad("this pairing code is invalid or has expired"))?;
    if consumed_at.is_some() {
        return Err(AppError::bad("this pairing code has already been used"));
    }
    let (Some(user_id), Some(shop_id)) = (approved_by, shop_id) else {
        let status = if expires_at < Utc::now() { "expired" } else { "pending" };
        return Ok(Json(json!({ "status": status })));
    };

    // Serialise terminal-code allocation per shop.
    let shop_name: String = sqlx::query_scalar("SELECT name FROM shops WHERE id = $1 FOR UPDATE").bind(shop_id).fetch_one(&mut *tx).await?;
    let token = format!("{TOKEN_PREFIX}{}", random_hex());
    let hash = sha256_hex(&token);
    // Re-pairing the same installation keeps its terminal code (and so its receipt series).
    let existing: Option<Uuid> = sqlx::query_scalar(
        "UPDATE pos_devices SET token_hash = $3, user_id = $4, name = $5, platform = $6, app_version = $7, last_seen_at = now()
         WHERE shop_id = $1 AND install_id = $2 AND revoked_at IS NULL RETURNING id",
    )
    .bind(shop_id)
    .bind(&install_id)
    .bind(&hash)
    .bind(user_id)
    .bind(&name)
    .bind(&platform)
    .bind(&app_version)
    .fetch_optional(&mut *tx)
    .await?;
    let device_id = match existing {
        Some(id) => id,
        None => {
            let n: i32 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(NULLIF(regexp_replace(code, '\\D', '', 'g'), '')::int), 0) + 1 FROM pos_devices WHERE shop_id = $1",
            )
            .bind(shop_id)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query_scalar(
                "INSERT INTO pos_devices (shop_id, user_id, code, name, platform, app_version, install_id, token_hash, last_seen_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8, now()) RETURNING id",
            )
            .bind(shop_id)
            .bind(user_id)
            .bind(format!("T{n:02}"))
            .bind(&name)
            .bind(&platform)
            .bind(&app_version)
            .bind(&install_id)
            .bind(&hash)
            .fetch_one(&mut *tx)
            .await?
        }
    };
    sqlx::query("UPDATE pos_pairings SET consumed_at = now() WHERE id = $1").bind(req.pair_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({
        "status": "approved",
        "token": token,
        "device_id": device_id,
        "shop": { "id": shop_id, "name": shop_name }
    })))
}

/// Owner: the shop's terminals.
pub async fn list_devices(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, shop_id, &user).await?;
    type Row = (Uuid, String, String, String, String, Option<String>, DateTime<Utc>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, Option<DateTime<Utc>>, i64, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT d.id, d.code, d.name, d.platform, d.app_version, u.display_name, d.created_at, d.last_seen_at,
                d.last_push_at, d.revoked_at,
                (SELECT COUNT(*) FROM pos_sales s WHERE s.device_id = d.id),
                (SELECT s.number FROM pos_sales s WHERE s.device_id = d.id ORDER BY s.device_seq DESC LIMIT 1)
         FROM pos_devices d LEFT JOIN users u ON u.id = d.user_id
         WHERE d.shop_id = $1 ORDER BY d.revoked_at IS NOT NULL, d.code",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, code, name, platform, version, user, created, seen, pushed, revoked, sales, last)| {
                json!({ "id": id, "code": code, "name": name, "platform": platform, "app_version": version,
                        "user": user, "created_at": created, "last_seen_at": seen, "last_push_at": pushed,
                        "revoked_at": revoked, "sales": sales, "last_receipt": last })
            })
            .collect(),
    ))
}

/// Owner: stop a terminal from syncing (lost laptop, staff left). Sales already synced stay.
pub async fn revoke(State(st): State<AppState>, user: AuthUser, Path((shop_id, device_id)): Path<(Uuid, Uuid)>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let done = sqlx::query("UPDATE pos_devices SET revoked_at = now() WHERE id = $1 AND shop_id = $2 AND revoked_at IS NULL")
        .bind(device_id)
        .bind(shop_id)
        .execute(&st.db)
        .await?
        .rows_affected();
    if done == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        let c = new_code();
        assert_eq!(c.len(), 8);
        assert!(c.bytes().all(|b| CODE_ALPHABET.contains(&b)));
        assert_eq!(normalize_code(" abcd-2345 "), "ABCD2345");
        assert_eq!(pretty("ABCD2345"), "ABCD-2345");
    }

    #[test]
    fn routes_are_scoped() {
        use axum::http::Method;
        // Approving needs POS access; terminal management stays with the owner.
        assert_eq!(crate::access::required(&Method::POST, "/pos/pair/approve"), Some("pos"));
        assert_eq!(crate::access::required(&Method::GET, "/shops/{id}/pos-devices"), None);
        assert_eq!(crate::access::required(&Method::POST, "/shops/{id}/pos-devices/{device_id}/revoke"), None);
    }
}
