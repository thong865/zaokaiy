//! Two-factor authentication (authenticator app + recovery codes).
//!
//! * Setup: `POST /auth/2fa/setup` returns a new secret (QR code + text) kept as *pending*;
//!   `POST /auth/2fa/enable` confirms it with a first code and returns 10 recovery codes, once.
//! * Sign-in: every way of signing in (password, Google, Facebook, WhatsApp) goes through
//!   [`sign_in`]. When 2FA is on it answers `{ mfa_required, mfa_token }` instead of a session;
//!   `POST /auth/2fa/verify` swaps that token + a code for the session.
//! * Wrong codes count per user: after [`MAX_FAILURES`] the account's second factor is locked for
//!   [`LOCK_MINUTES`], so a stolen password can't be used to brute-force the 6 digits.

use axum::{extract::State, Json};
use chrono::Utc;
use qrcode::{render::svg, QrCode};
use rand::{Rng, RngCore};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    auth::{issue_token, AuthUser},
    error::{AppError, AppResult},
    models::User,
    totp, AppState,
};

const CHALLENGE_TTL: &str = "5 minutes";
const CHALLENGE_TTL_SECS: i64 = 300;
const MAX_FAILURES: i32 = 5;
const LOCK_MINUTES: i32 = 15;
const RECOVERY_CODES: usize = 10;
const ISSUER: &str = "zaokaiy";

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn recovery_hash(st: &AppState, code: &str) -> String {
    sha256_hex(&format!("recovery:{code}:{}", st.cfg.jwt_secret))
}

/// "7kq2m-x9fdt" — 10 characters without look-alikes (0/o, 1/l/i).
fn new_recovery_code() -> String {
    const CHARS: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";
    let mut rng = rand::thread_rng();
    let s: String = (0..10).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect();
    format!("{}-{}", &s[..5], &s[5..])
}

fn normalize_recovery(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect::<String>()
}

// ---------------------------------------------------------------------------------------------
// Sign-in gate
// ---------------------------------------------------------------------------------------------

/// Finish a sign-in that passed the first factor: a session, or a 2FA challenge when the account
/// has 2FA on. `linked` is set when a signed-in user connected a login method (no challenge then).
pub async fn sign_in(st: &AppState, user: User, is_new: bool, linked: Option<String>, redirect: Option<String>) -> AppResult<Json<Value>> {
    if linked.is_none() && user.totp_enabled_at.is_some() {
        let mut raw = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut raw);
        let token = hex::encode(raw);
        sqlx::query(&format!("DELETE FROM mfa_challenges WHERE created_at < now() - interval '{CHALLENGE_TTL}'")).execute(&st.db).await?;
        sqlx::query("INSERT INTO mfa_challenges (token_hash, user_id, is_new, redirect_to) VALUES ($1,$2,$3,$4)")
            .bind(sha256_hex(&token))
            .bind(user.id)
            .bind(is_new)
            .bind(&redirect)
            .execute(&st.db)
            .await?;
        return Ok(Json(json!({ "mfa_required": true, "mfa_token": token, "expires_in": CHALLENGE_TTL_SECS, "redirect": redirect })));
    }
    let token = issue_token(st, user.id, &user.role)?;
    Ok(Json(json!({ "token": token, "user": user, "is_new": is_new, "linked": linked, "redirect": redirect })))
}

#[derive(Deserialize)]
pub struct VerifyReq {
    pub mfa_token: String,
    pub code: String,
}

/// Second step of a sign-in: challenge token + authenticator or recovery code → session.
pub async fn verify(State(st): State<AppState>, Json(req): Json<VerifyReq>) -> AppResult<Json<Value>> {
    let hash = sha256_hex(req.mfa_token.trim());
    let row: Option<(Uuid,)> = sqlx::query_as(&format!(
        "SELECT user_id FROM mfa_challenges WHERE token_hash = $1 AND created_at > now() - interval '{CHALLENGE_TTL}'"
    ))
    .bind(&hash)
    .fetch_optional(&st.db)
    .await?;
    let (uid,) = row.ok_or_else(|| AppError::bad("this sign-in attempt has expired, please sign in again"))?;
    check_code(&st, uid, &req.code).await?;
    let row: Option<(bool, Option<String>)> = sqlx::query_as("DELETE FROM mfa_challenges WHERE token_hash = $1 RETURNING is_new, redirect_to")
        .bind(&hash)
        .fetch_optional(&st.db)
        .await?;
    let (is_new, redirect) = row.ok_or_else(|| AppError::bad("this sign-in attempt has expired, please sign in again"))?;
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(uid).fetch_one(&st.db).await?;
    let token = issue_token(&st, user.id, &user.role)?;
    Ok(Json(json!({ "token": token, "user": user, "is_new": is_new, "linked": null, "redirect": redirect })))
}

/// Accept a current authenticator code or an unused recovery code, with lockout after repeated
/// failures. Errors when the code is wrong.
async fn check_code(st: &AppState, uid: Uuid, code: &str) -> AppResult<()> {
    let (secret, last_step, locked): (Option<String>, Option<i64>, bool) = sqlx::query_as(
        "SELECT totp_secret, totp_last_step, coalesce(totp_locked_until > now(), false) FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_one(&st.db)
    .await?;
    let secret = secret.ok_or_else(|| AppError::bad("two-factor authentication is not turned on"))?;
    if locked {
        return Err(AppError::bad("too many wrong codes, try again in 15 minutes"));
    }
    let digits: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    let ok = if digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_digit()) {
        let key = hex::decode(st.sealer.open_str(&secret)?).map_err(|_| AppError::Internal("2fa secret is corrupt".into()))?;
        match totp::verify(&key, &digits, Utc::now().timestamp(), last_step) {
            // Conditional update: two requests racing with the same code can't both win.
            Some(step) => sqlx::query("UPDATE users SET totp_last_step = $2 WHERE id = $1 AND (totp_last_step IS NULL OR totp_last_step < $2)")
                .bind(uid)
                .bind(step)
                .execute(&st.db)
                .await?
                .rows_affected() == 1,
            None => false,
        }
    } else {
        let norm = normalize_recovery(code);
        norm.len() == 10
            && sqlx::query("UPDATE user_recovery_codes SET used_at = now() WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL")
                .bind(uid)
                .bind(recovery_hash(st, &norm))
                .execute(&st.db)
                .await?
                .rows_affected() == 1
    };
    if ok {
        sqlx::query("UPDATE users SET totp_failures = 0, totp_locked_until = NULL WHERE id = $1").bind(uid).execute(&st.db).await?;
        return Ok(());
    }
    sqlx::query(&format!(
        "UPDATE users SET totp_failures = CASE WHEN totp_failures + 1 >= {MAX_FAILURES} THEN 0 ELSE totp_failures + 1 END,
                          totp_locked_until = CASE WHEN totp_failures + 1 >= {MAX_FAILURES} THEN now() + interval '{LOCK_MINUTES} minutes' ELSE totp_locked_until END
         WHERE id = $1"
    ))
    .bind(uid)
    .execute(&st.db)
    .await?;
    Err(AppError::bad("wrong code"))
}

async fn replace_recovery_codes(st: &AppState, uid: Uuid) -> AppResult<Vec<String>> {
    let codes: Vec<String> = (0..RECOVERY_CODES).map(|_| new_recovery_code()).collect();
    let hashes: Vec<String> = codes.iter().map(|c| recovery_hash(st, &normalize_recovery(c))).collect();
    let mut tx = st.db.begin().await?;
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = $1").bind(uid).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO user_recovery_codes (user_id, code_hash) SELECT $1, unnest($2::text[])")
        .bind(uid)
        .bind(&hashes)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(codes)
}

// ---------------------------------------------------------------------------------------------
// Account settings
// ---------------------------------------------------------------------------------------------

pub async fn status(State(st): State<AppState>, user: AuthUser) -> AppResult<Json<Value>> {
    let (enabled_at, left): (Option<chrono::DateTime<Utc>>, i64) = sqlx::query_as(
        "SELECT u.totp_enabled_at, (SELECT count(*) FROM user_recovery_codes r WHERE r.user_id = u.id AND r.used_at IS NULL)
         FROM users u WHERE u.id = $1",
    )
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({ "enabled": enabled_at.is_some(), "enabled_at": enabled_at, "recovery_codes_left": left })))
}

/// New secret for the authenticator app (not active until confirmed with `enable`).
pub async fn setup(State(st): State<AppState>, user: AuthUser) -> AppResult<Json<Value>> {
    let u: User = sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(user.id).fetch_one(&st.db).await?;
    if u.totp_enabled_at.is_some() {
        return Err(AppError::Conflict("two-factor authentication is already on".into()));
    }
    let secret = totp::generate_secret();
    let b32 = totp::base32(&secret);
    sqlx::query("UPDATE users SET totp_pending_secret = $2 WHERE id = $1")
        .bind(user.id)
        .bind(st.sealer.seal_str(&hex::encode(&secret)))
        .execute(&st.db)
        .await?;
    let account = u.email.or(u.phone).unwrap_or(u.display_name);
    let uri = totp::otpauth_uri(ISSUER, &account, &b32);
    let qr = QrCode::new(uri.as_bytes())
        .map_err(|e| AppError::Internal(e.to_string()))?
        .render::<svg::Color>()
        .min_dimensions(200, 200)
        .quiet_zone(true)
        .build();
    // Groups of 4 are easier to type by hand.
    let grouped = b32.as_bytes().chunks(4).map(|c| std::str::from_utf8(c).unwrap()).collect::<Vec<_>>().join(" ");
    Ok(Json(json!({ "secret": grouped, "otpauth_url": uri, "qr_svg": qr })))
}

#[derive(Deserialize)]
pub struct CodeReq {
    pub code: String,
}

/// Confirm setup with the first code from the app. Returns the recovery codes (shown once).
pub async fn enable(State(st): State<AppState>, user: AuthUser, Json(req): Json<CodeReq>) -> AppResult<Json<Value>> {
    let (pending, enabled): (Option<String>, bool) =
        sqlx::query_as("SELECT totp_pending_secret, totp_enabled_at IS NOT NULL FROM users WHERE id = $1").bind(user.id).fetch_one(&st.db).await?;
    if enabled {
        return Err(AppError::Conflict("two-factor authentication is already on".into()));
    }
    let pending = pending.ok_or_else(|| AppError::bad("start the setup again"))?;
    let key = hex::decode(st.sealer.open_str(&pending)?).map_err(|_| AppError::Internal("2fa secret is corrupt".into()))?;
    let code: String = req.code.chars().filter(|c| c.is_ascii_digit()).collect();
    let step = totp::verify(&key, &code, Utc::now().timestamp(), None).ok_or_else(|| AppError::bad("wrong code"))?;
    sqlx::query(
        "UPDATE users SET totp_secret = totp_pending_secret, totp_pending_secret = NULL, totp_enabled_at = now(),
                          totp_last_step = $2, totp_failures = 0, totp_locked_until = NULL
         WHERE id = $1",
    )
    .bind(user.id)
    .bind(step)
    .execute(&st.db)
    .await?;
    let codes = replace_recovery_codes(&st, user.id).await?;
    tracing::info!(user = %user.id, "two-factor authentication enabled");
    Ok(Json(json!({ "enabled": true, "recovery_codes": codes })))
}

/// Turn 2FA off — needs a current code (or a recovery code).
pub async fn disable(State(st): State<AppState>, user: AuthUser, Json(req): Json<CodeReq>) -> AppResult<Json<Value>> {
    check_code(&st, user.id, &req.code).await?;
    sqlx::query(
        "UPDATE users SET totp_secret = NULL, totp_pending_secret = NULL, totp_enabled_at = NULL, totp_last_step = NULL,
                          totp_failures = 0, totp_locked_until = NULL
         WHERE id = $1",
    )
    .bind(user.id)
    .execute(&st.db)
    .await?;
    sqlx::query("DELETE FROM user_recovery_codes WHERE user_id = $1").bind(user.id).execute(&st.db).await?;
    sqlx::query("DELETE FROM mfa_challenges WHERE user_id = $1").bind(user.id).execute(&st.db).await?;
    tracing::info!(user = %user.id, "two-factor authentication disabled");
    Ok(Json(json!({ "enabled": false })))
}

/// New set of recovery codes (the old ones stop working) — needs a current code.
pub async fn regenerate(State(st): State<AppState>, user: AuthUser, Json(req): Json<CodeReq>) -> AppResult<Json<Value>> {
    check_code(&st, user.id, &req.code).await?;
    let codes = replace_recovery_codes(&st, user.id).await?;
    Ok(Json(json!({ "recovery_codes": codes })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_codes() {
        let c = new_recovery_code();
        assert_eq!(c.len(), 11);
        assert_eq!(&c[5..6], "-");
        assert_eq!(normalize_recovery(&c).len(), 10);
        assert_eq!(normalize_recovery(" 7KQ2M-x9fdt "), "7kq2mx9fdt");
    }
}
