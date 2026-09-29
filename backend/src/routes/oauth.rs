//! Social login.
//!
//! * **Google** and **Facebook**: OAuth 2.0 authorization-code flow run by the API.
//!   `POST /auth/oauth/{provider}/start` returns the provider URL (state + PKCE stored in
//!   `oauth_states`); the provider redirects to `GET /auth/oauth/{provider}/callback`, which
//!   exchanges the code server-to-server, finds or creates the user and sends the browser to
//!   `PUBLIC_WEB_URL/auth/callback?code=<one-time>`. The web app swaps that code for a session
//!   token with `POST /auth/exchange`, so a token never travels in a URL.
//! * **WhatsApp**: a 6-digit code sent with a WhatsApp Cloud API *authentication* template
//!   (`POST /auth/whatsapp/send`, then `POST /auth/whatsapp/verify`).
//!
//! Account linking: a Google identity with a verified email joins the account with that email.
//! Facebook emails are not treated as verified — if the email already belongs to an account the
//! user is asked to sign in and connect Facebook from their account page. Signed-in users can
//! connect/disconnect methods (`link: true`, `GET/DELETE /auth/identities`).

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Redirect,
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use rand::{distributions::Alphanumeric, Rng, RngCore};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    auth::{hash_password, optional_user, verify_password, AuthUser},
    captcha,
    config::Config,
    error::{AppError, AppResult},
    models::User,
    AppState,
};

const STATE_TTL: &str = "10 minutes";
const LOGIN_CODE_TTL: &str = "2 minutes";
const OTP_TTL_SECS: i64 = 600;
const OTP_MAX_PER_HOUR: i64 = 5;
const OTP_MAX_ATTEMPTS: i32 = 5;

fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Constant-time comparison of two hex digests.
fn ct_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn google_on(c: &Config) -> bool {
    !c.google_client_id.is_empty() && !c.google_client_secret.is_empty()
}
fn facebook_on(c: &Config) -> bool {
    !c.facebook_app_id.is_empty() && !c.facebook_app_secret.is_empty()
}
fn whatsapp_configured(c: &Config) -> bool {
    !c.whatsapp_token.is_empty() && !c.whatsapp_phone_number_id.is_empty()
}
fn whatsapp_on(c: &Config) -> bool {
    whatsapp_configured(c) || c.otp_dev_echo
}

fn providers_json(c: &Config) -> Value {
    let captcha_site_key = (!c.turnstile_site_key.is_empty() && !c.turnstile_secret_key.is_empty()).then_some(c.turnstile_site_key.as_str());
    json!({ "password": true, "google": google_on(c), "facebook": facebook_on(c), "whatsapp": whatsapp_on(c), "captcha_site_key": captcha_site_key })
}

/// Which login methods are available (the login page shows only these buttons).
pub async fn providers(State(st): State<AppState>) -> Json<Value> {
    Json(providers_json(&st.cfg))
}

fn callback_url(c: &Config, provider: &str) -> String {
    format!("{}/auth/oauth/{provider}/callback", c.public_api_url.trim_end_matches('/'))
}

/// Only same-site relative paths ("/dashboard?x=1"); never "//host", "/\host" or absolute URLs.
fn safe_redirect(r: Option<&str>) -> String {
    match r {
        Some(p) if p.starts_with('/') && !p.starts_with("//") && !p.contains('\\') && p.len() <= 512 => p.to_string(),
        _ => "/".into(),
    }
}

/// E.164 from what the user typed. The web app sends "+<country><number>"; "00" prefix accepted.
pub fn normalize_phone(raw: &str) -> Option<String> {
    let t = raw.trim();
    let intl = t.starts_with('+') || t.starts_with("00");
    let mut d: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    if t.starts_with("00") {
        d = d[2..].to_string();
    }
    if !intl || d.starts_with('0') || !(8..=15).contains(&d.len()) {
        return None;
    }
    Some(format!("+{d}"))
}

// ---------------------------------------------------------------------------------------------
// OAuth (Google, Facebook)
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct StartReq {
    pub redirect: Option<String>,
    /// Connect this provider to the signed-in account instead of signing in.
    #[serde(default)]
    pub link: bool,
}

pub async fn start(
    State(st): State<AppState>,
    Path(provider): Path<String>,
    headers: HeaderMap,
    Json(req): Json<StartReq>,
) -> AppResult<Json<Value>> {
    let c = &st.cfg;
    let enabled = match provider.as_str() {
        "google" => google_on(c),
        "facebook" => facebook_on(c),
        _ => return Err(AppError::NotFound),
    };
    if !enabled {
        return Err(AppError::bad("this login method is not enabled"));
    }
    let link_user = if req.link { Some(optional_user(&st, &headers).ok_or(AppError::Unauthorized)?.id) } else { None };

    sqlx::query(&format!("DELETE FROM oauth_states WHERE created_at < now() - interval '{STATE_TTL}'"))
        .execute(&st.db)
        .await?;
    let state = random_hex(24);
    let verifier: String = rand::thread_rng().sample_iter(&Alphanumeric).take(64).map(char::from).collect();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    sqlx::query("INSERT INTO oauth_states (state, provider, code_verifier, redirect_to, link_user_id) VALUES ($1,$2,$3,$4,$5)")
        .bind(&state)
        .bind(&provider)
        .bind(&verifier)
        .bind(safe_redirect(req.redirect.as_deref()))
        .bind(link_user)
        .execute(&st.db)
        .await?;

    let redirect_uri = callback_url(c, &provider);
    let url = if provider == "google" {
        Url::parse_with_params(
            &c.google_auth_url,
            &[
                ("client_id", c.google_client_id.as_str()),
                ("redirect_uri", &redirect_uri),
                ("response_type", "code"),
                ("scope", "openid email profile"),
                ("state", &state),
                ("code_challenge", &challenge),
                ("code_challenge_method", "S256"),
                ("prompt", "select_account"),
            ],
        )
    } else {
        Url::parse_with_params(
            &c.facebook_dialog_url,
            &[
                ("client_id", c.facebook_app_id.as_str()),
                ("redirect_uri", &redirect_uri),
                ("response_type", "code"),
                ("scope", "public_profile,email"),
                ("state", &state),
            ],
        )
    }
    .map_err(|e| AppError::Internal(e.to_string()))?;
    Ok(Json(json!({ "url": url.to_string() })))
}

/// What we learn about the person from a provider.
struct Profile {
    subject: String,
    email: Option<String>,
    /// The provider vouches that the person controls `email` (Google `email_verified`).
    email_verified: bool,
    name: String,
    avatar: Option<String>,
    /// Verified phone (WhatsApp code).
    phone: Option<String>,
}

async fn google_profile(st: &AppState, code: &str, verifier: &str) -> Result<Profile, String> {
    let c = &st.cfg;
    let redirect_uri = callback_url(c, "google");
    let res = st
        .http
        .post(&c.google_token_url)
        .form(&[
            ("code", code),
            ("client_id", &c.google_client_id),
            ("client_secret", &c.google_client_secret),
            ("redirect_uri", &redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = res.status();
    let tok: Value = res.json().await.map_err(|e| e.to_string())?;
    let access = tok["access_token"].as_str().ok_or_else(|| format!("token exchange failed ({status}): {tok}"))?;
    let info: Value = st
        .http
        .get(&c.google_userinfo_url)
        .bearer_auth(access)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let subject = info["sub"].as_str().ok_or_else(|| format!("userinfo without sub: {info}"))?.to_string();
    Ok(Profile {
        subject,
        email: info["email"].as_str().map(|e| e.trim().to_lowercase()),
        email_verified: info["email_verified"].as_bool().unwrap_or(false),
        name: info["name"].as_str().unwrap_or_default().trim().to_string(),
        avatar: info["picture"].as_str().map(String::from),
        phone: None,
    })
}

async fn facebook_profile(st: &AppState, code: &str) -> Result<Profile, String> {
    let c = &st.cfg;
    let graph = c.meta_graph_url.trim_end_matches('/');
    let redirect_uri = callback_url(c, "facebook");
    let tok: Value = st
        .http
        .get(format!("{graph}/oauth/access_token"))
        .query(&[
            ("client_id", c.facebook_app_id.as_str()),
            ("client_secret", &c.facebook_app_secret),
            ("redirect_uri", &redirect_uri),
            ("code", code),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let access = tok["access_token"].as_str().ok_or_else(|| format!("token exchange failed: {tok}"))?;
    // appsecret_proof proves the call comes from our server (recommended by Meta).
    let mut mac = Hmac::<Sha256>::new_from_slice(c.facebook_app_secret.as_bytes()).map_err(|e| e.to_string())?;
    mac.update(access.as_bytes());
    let proof = hex::encode(mac.finalize().into_bytes());
    let me: Value = st
        .http
        .get(format!("{graph}/me"))
        .query(&[
            ("fields", "id,name,email,picture.width(256).height(256)"),
            ("access_token", access),
            ("appsecret_proof", &proof),
        ])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let subject = me["id"].as_str().ok_or_else(|| format!("profile without id: {me}"))?.to_string();
    Ok(Profile {
        subject,
        email: me["email"].as_str().map(|e| e.trim().to_lowercase()),
        email_verified: false,
        name: me["name"].as_str().unwrap_or_default().trim().to_string(),
        avatar: me["picture"]["data"]["url"].as_str().map(String::from),
        phone: None,
    })
}

#[derive(Deserialize)]
pub struct CallbackQ {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Provider → here → web app. Errors go to the web app as `?error=<code>&provider=<p>`:
/// cancelled, expired, provider, email_exists, already_linked, server.
pub async fn callback(State(st): State<AppState>, Path(provider): Path<String>, Query(q): Query<CallbackQ>) -> Redirect {
    let web = st.cfg.public_web_url.trim_end_matches('/').to_string();
    match finish_oauth(&st, &provider, q).await {
        Ok(code) => Redirect::to(&format!("{web}/auth/callback?code={code}")),
        Err(err) => {
            let p = if matches!(provider.as_str(), "google" | "facebook") { provider.as_str() } else { "" };
            Redirect::to(&format!("{web}/auth/callback?error={err}&provider={p}"))
        }
    }
}

async fn finish_oauth(st: &AppState, provider: &str, q: CallbackQ) -> Result<String, &'static str> {
    if q.error.is_some() {
        return Err("cancelled");
    }
    let code = q.code.ok_or("cancelled")?;
    let state = q.state.ok_or("expired")?;
    let row: Option<(String, String, Option<Uuid>)> = sqlx::query_as(&format!(
        "DELETE FROM oauth_states WHERE state = $1 AND provider = $2 AND created_at > now() - interval '{STATE_TTL}'
         RETURNING code_verifier, redirect_to, link_user_id"
    ))
    .bind(&state)
    .bind(provider)
    .fetch_optional(&st.db)
    .await
    .map_err(|_| "server")?;
    let (verifier, redirect_to, link_user) = row.ok_or("expired")?;
    let profile = match provider {
        "google" => google_profile(st, &code, &verifier).await,
        "facebook" => facebook_profile(st, &code).await,
        _ => Err("unknown provider".into()),
    }
    .map_err(|e| {
        tracing::warn!(provider, error = %e, "oauth: provider exchange failed");
        "provider"
    })?;
    let (user_id, is_new, linked) = resolve_user(st, provider, &profile, link_user).await?;
    let one_time = random_hex(32);
    sqlx::query(&format!("DELETE FROM login_codes WHERE created_at < now() - interval '{LOGIN_CODE_TTL}'"))
        .execute(&st.db)
        .await
        .map_err(|_| "server")?;
    sqlx::query("INSERT INTO login_codes (code_hash, user_id, is_new, linked, redirect_to) VALUES ($1,$2,$3,$4,$5)")
        .bind(sha256_hex(&one_time))
        .bind(user_id)
        .bind(is_new)
        .bind(linked)
        .bind(&redirect_to)
        .execute(&st.db)
        .await
        .map_err(|_| "server")?;
    Ok(one_time)
}

/// Find or create the user behind a provider identity. Returns (user id, created?, linked provider).
async fn resolve_user(
    st: &AppState,
    provider: &str,
    p: &Profile,
    link_user: Option<Uuid>,
) -> Result<(Uuid, bool, Option<String>), &'static str> {
    let db = |_| "server";
    let mut tx = st.db.begin().await.map_err(db)?;

    // 1. Known identity → that account.
    let known: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM user_identities WHERE provider = $1 AND subject = $2")
        .bind(provider)
        .bind(&p.subject)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
    if let Some(uid) = known {
        if link_user.is_some_and(|l| l != uid) {
            return Err("already_linked");
        }
        sqlx::query("UPDATE user_identities SET last_login_at = now(), email = COALESCE($3, email), name = CASE WHEN $4 = '' THEN name ELSE $4 END WHERE provider = $1 AND subject = $2")
            .bind(provider)
            .bind(&p.subject)
            .bind(&p.email)
            .bind(&p.name)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("UPDATE users SET avatar_url = COALESCE(avatar_url, $2) WHERE id = $1")
            .bind(uid)
            .bind(&p.avatar)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit().await.map_err(db)?;
        return Ok((uid, false, link_user.map(|_| provider.to_string())));
    }

    // 2. Connecting to the signed-in account.
    let (uid, is_new) = if let Some(uid) = link_user {
        if let Some(phone) = &p.phone {
            let taken: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE phone = $1 AND id <> $2")
                .bind(phone)
                .bind(uid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db)?;
            if taken.is_some() {
                return Err("already_linked");
            }
            sqlx::query("UPDATE users SET phone = $2 WHERE id = $1").bind(uid).bind(phone).execute(&mut *tx).await.map_err(db)?;
        }
        (uid, false)
    } else {
        // 3. Same person by verified email (Google) or verified phone (WhatsApp).
        let mut existing: Option<Uuid> = None;
        if let Some(phone) = &p.phone {
            existing = sqlx::query_scalar("SELECT id FROM users WHERE phone = $1").bind(phone).fetch_optional(&mut *tx).await.map_err(db)?;
        }
        if existing.is_none() {
            if let Some(email) = &p.email {
                let by_email: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE lower(email) = $1")
                    .bind(email)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(db)?;
                if by_email.is_some() && !p.email_verified {
                    return Err("email_exists");
                }
                existing = by_email;
            }
        }
        match existing {
            Some(uid) => (uid, false),
            None => {
                // 4. New account. Only a provider-verified email is stored on the user.
                let email = p.email.as_ref().filter(|_| p.email_verified);
                let name = if !p.name.is_empty() {
                    p.name.clone()
                } else if let Some(ph) = &p.phone {
                    format!("WhatsApp {}", &ph[ph.len().saturating_sub(4)..])
                } else {
                    "zaokaiy user".into()
                };
                let admin = email.is_some_and(|e| st.cfg.admin_emails.contains(e));
                let uid: Uuid = sqlx::query_scalar(
                    "INSERT INTO users (email, password_hash, display_name, role, phone, avatar_url)
                     VALUES ($1, NULL, $2, $3, $4, $5) RETURNING id",
                )
                .bind(email)
                .bind(name.chars().take(80).collect::<String>())
                .bind(if admin { "admin" } else { "user" })
                .bind(&p.phone)
                .bind(&p.avatar)
                .fetch_one(&mut *tx)
                .await
                .map_err(db)?;
                (uid, true)
            }
        }
    };

    let inserted = sqlx::query(
        "INSERT INTO user_identities (user_id, provider, subject, email, name) VALUES ($1,$2,$3,$4,$5)
         ON CONFLICT (user_id, provider) DO NOTHING",
    )
    .bind(uid)
    .bind(provider)
    .bind(&p.subject)
    .bind(&p.email)
    .bind(&p.name)
    .execute(&mut *tx)
    .await
    .map_err(db)?;
    if inserted.rows_affected() == 0 {
        // The account already has a different identity from this provider.
        return Err("already_linked");
    }
    tx.commit().await.map_err(db)?;
    Ok((uid, is_new, link_user.map(|_| provider.to_string())))
}

#[derive(Deserialize)]
pub struct ExchangeReq {
    pub code: String,
}

/// One-time code from the OAuth callback → session token.
pub async fn exchange(State(st): State<AppState>, Json(req): Json<ExchangeReq>) -> AppResult<Json<Value>> {
    let row: Option<(Uuid, bool, Option<String>, String)> = sqlx::query_as(&format!(
        "DELETE FROM login_codes WHERE code_hash = $1 AND created_at > now() - interval '{LOGIN_CODE_TTL}'
         RETURNING user_id, is_new, linked, redirect_to"
    ))
    .bind(sha256_hex(req.code.trim()))
    .fetch_optional(&st.db)
    .await?;
    let (uid, is_new, linked, redirect) = row.ok_or_else(|| AppError::bad("this sign-in link has expired, please try again"))?;
    session(&st, uid, is_new, linked, Some(redirect)).await
}

async fn session(st: &AppState, uid: Uuid, is_new: bool, linked: Option<String>, redirect: Option<String>) -> AppResult<Json<Value>> {
    let user: User = sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(uid).fetch_one(&st.db).await?;
    super::two_factor::sign_in(st, user, is_new, linked, redirect).await
}

// ---------------------------------------------------------------------------------------------
// WhatsApp one-time code
// ---------------------------------------------------------------------------------------------

fn otp_hash(c: &Config, phone: &str, code: &str) -> String {
    sha256_hex(&format!("{phone}:{code}:{}", c.jwt_secret))
}

#[derive(Deserialize)]
pub struct WaSendReq {
    pub phone: String,
    /// UI language ("lo" picks WHATSAPP_OTP_LANG_LO when set).
    pub lang: Option<String>,
    /// Cloudflare Turnstile token.
    pub captcha: Option<String>,
}

pub async fn whatsapp_send(State(st): State<AppState>, headers: HeaderMap, Json(req): Json<WaSendReq>) -> AppResult<Json<Value>> {
    let c = &st.cfg;
    if !whatsapp_on(c) {
        return Err(AppError::bad("this login method is not enabled"));
    }
    captcha::verify(&st, &headers, req.captcha.as_deref()).await?;
    let phone = normalize_phone(&req.phone).ok_or_else(|| AppError::bad("enter a valid phone number with country code"))?;
    let (since_last, last_hour): (Option<f64>, i64) = sqlx::query_as(
        "SELECT EXTRACT(EPOCH FROM now() - max(created_at))::float8, count(*)
         FROM phone_otps WHERE phone = $1 AND created_at > now() - interval '1 hour'",
    )
    .bind(&phone)
    .fetch_one(&st.db)
    .await?;
    if let Some(s) = since_last.filter(|s| *s < c.otp_resend_secs) {
        return Err(AppError::bad(format!("please wait {} seconds before requesting another code", (c.otp_resend_secs - s).ceil() as i64)));
    }
    if last_hour >= OTP_MAX_PER_HOUR {
        return Err(AppError::bad("too many codes requested, try again in an hour"));
    }
    let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
    sqlx::query("DELETE FROM phone_otps WHERE created_at < now() - interval '1 day'").execute(&st.db).await?;
    let id: Uuid = sqlx::query_scalar("INSERT INTO phone_otps (phone, code_hash) VALUES ($1,$2) RETURNING id")
        .bind(&phone)
        .bind(otp_hash(c, &phone, &code))
        .fetch_one(&st.db)
        .await?;

    if whatsapp_configured(c) {
        let lang = if req.lang.as_deref() == Some("lo") && !c.whatsapp_otp_lang_lo.is_empty() {
            c.whatsapp_otp_lang_lo.as_str()
        } else {
            c.whatsapp_otp_lang.as_str()
        };
        // Authentication template with a one-time-password (copy code) button.
        let body = json!({
            "messaging_product": "whatsapp",
            "to": phone.trim_start_matches('+'),
            "type": "template",
            "template": {
                "name": c.whatsapp_otp_template,
                "language": { "code": lang },
                "components": [
                    { "type": "body", "parameters": [{ "type": "text", "text": code }] },
                    { "type": "button", "sub_type": "url", "index": "0", "parameters": [{ "type": "text", "text": code }] }
                ]
            }
        });
        let url = format!("{}/{}/messages", c.meta_graph_url.trim_end_matches('/'), c.whatsapp_phone_number_id);
        let res = st.http.post(&url).bearer_auth(&c.whatsapp_token).json(&body).send().await;
        let err = match res {
            Ok(r) if r.status().is_success() => None,
            Ok(r) => {
                let v: Value = r.json().await.unwrap_or(Value::Null);
                Some(v["error"]["message"].as_str().unwrap_or("request rejected").to_string())
            }
            Err(e) => Some(e.to_string()),
        };
        if let Some(e) = err {
            sqlx::query("DELETE FROM phone_otps WHERE id = $1").bind(id).execute(&st.db).await?;
            tracing::warn!(error = %e, "whatsapp otp send failed");
            return Err(AppError::Upstream(format!("could not send the WhatsApp message: {e}")));
        }
    }
    let mut out = json!({ "sent": true, "phone": phone, "expires_in": OTP_TTL_SECS, "resend_in": c.otp_resend_secs as i64 });
    if c.otp_dev_echo {
        tracing::warn!(%phone, "OTP_DEV_ECHO is on: returning the login code in the response (never enable in production)");
        out["dev_code"] = json!(code);
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
pub struct WaVerifyReq {
    pub phone: String,
    pub code: String,
    /// Name for a new account (optional).
    pub display_name: Option<String>,
    #[serde(default)]
    pub link: bool,
}

pub async fn whatsapp_verify(State(st): State<AppState>, headers: HeaderMap, Json(req): Json<WaVerifyReq>) -> AppResult<Json<Value>> {
    let phone = normalize_phone(&req.phone).ok_or_else(|| AppError::bad("enter a valid phone number with country code"))?;
    let link_user = if req.link { Some(optional_user(&st, &headers).ok_or(AppError::Unauthorized)?.id) } else { None };
    let code: String = req.code.chars().filter(|c| c.is_ascii_digit()).collect();
    let row: Option<(Uuid, String, i32)> = sqlx::query_as(&format!(
        "SELECT id, code_hash, attempts FROM phone_otps
         WHERE phone = $1 AND consumed_at IS NULL AND created_at > now() - interval '{OTP_TTL_SECS} seconds'
         ORDER BY created_at DESC LIMIT 1"
    ))
    .bind(&phone)
    .fetch_optional(&st.db)
    .await?;
    let (id, hash, attempts) = row.ok_or_else(|| AppError::bad("the code has expired, request a new one"))?;
    if attempts >= OTP_MAX_ATTEMPTS {
        return Err(AppError::bad("too many wrong attempts, request a new code"));
    }
    if code.len() != 6 || !ct_eq(&hash, &otp_hash(&st.cfg, &phone, &code)) {
        sqlx::query("UPDATE phone_otps SET attempts = attempts + 1 WHERE id = $1").bind(id).execute(&st.db).await?;
        return Err(AppError::bad("wrong code"));
    }
    let consumed = sqlx::query("UPDATE phone_otps SET consumed_at = now() WHERE id = $1 AND consumed_at IS NULL")
        .bind(id)
        .execute(&st.db)
        .await?;
    if consumed.rows_affected() == 0 {
        return Err(AppError::bad("the code has expired, request a new one"));
    }
    let profile = Profile {
        subject: phone.clone(),
        email: None,
        email_verified: false,
        name: req.display_name.as_deref().unwrap_or_default().trim().to_string(),
        avatar: None,
        phone: Some(phone),
    };
    let (uid, is_new, linked) = resolve_user(&st, "whatsapp", &profile, link_user).await.map_err(|e| match e {
        "already_linked" => AppError::Conflict("this login is already connected to another account".into()),
        other => AppError::Internal(other.into()),
    })?;
    session(&st, uid, is_new, linked, None).await
}

// ---------------------------------------------------------------------------------------------
// Account: connected login methods, password
// ---------------------------------------------------------------------------------------------

pub async fn identities(State(st): State<AppState>, user: AuthUser) -> AppResult<Json<Value>> {
    let u: User = sqlx::query_as("SELECT * FROM users WHERE id = $1").bind(user.id).fetch_one(&st.db).await?;
    let rows: Vec<(String, Option<String>, String, DateTime<Utc>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT provider, email, name, created_at, last_login_at FROM user_identities WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    let list: Vec<Value> = rows
        .into_iter()
        .map(|(provider, email, name, created_at, last_login_at)| {
            json!({ "provider": provider, "email": email, "name": name, "created_at": created_at, "last_login_at": last_login_at })
        })
        .collect();
    Ok(Json(json!({
        "has_password": u.password_hash.is_some(),
        "email": u.email,
        "phone": u.phone,
        "identities": list,
        "providers": providers_json(&st.cfg),
    })))
}

pub async fn unlink(State(st): State<AppState>, user: AuthUser, Path(provider): Path<String>) -> AppResult<Json<Value>> {
    let exists: Option<Uuid> = sqlx::query_scalar("SELECT id FROM user_identities WHERE user_id = $1 AND provider = $2")
        .bind(user.id)
        .bind(&provider)
        .fetch_optional(&st.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    let (has_password, has_email, has_phone, count): (bool, bool, bool, i64) = sqlx::query_as(
        "SELECT u.password_hash IS NOT NULL, u.email IS NOT NULL, u.phone IS NOT NULL,
                (SELECT count(*) FROM user_identities i WHERE i.user_id = u.id)
         FROM users u WHERE u.id = $1",
    )
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    // A password only helps if there is still an e-mail or phone to sign in with afterwards.
    let password_usable = has_password && (has_email || (has_phone && provider != "whatsapp"));
    if !password_usable && count <= 1 {
        return Err(AppError::bad("add a password or another login method before disconnecting this one"));
    }
    let gone = sqlx::query("DELETE FROM user_identities WHERE user_id = $1 AND provider = $2")
        .bind(user.id)
        .bind(&provider)
        .execute(&st.db)
        .await?;
    if gone.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    if provider == "whatsapp" {
        sqlx::query("UPDATE users SET phone = NULL WHERE id = $1").bind(user.id).execute(&st.db).await?;
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PasswordReq {
    pub current: Option<String>,
    pub password: String,
}

/// Set a password (social-only accounts) or change it (current password required).
pub async fn set_password(State(st): State<AppState>, user: AuthUser, Json(req): Json<PasswordReq>) -> AppResult<Json<Value>> {
    if req.password.len() < 8 {
        return Err(AppError::bad("password must be at least 8 characters"));
    }
    let (hash, email, phone): (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as("SELECT password_hash, email, phone FROM users WHERE id = $1").bind(user.id).fetch_one(&st.db).await?;
    if let Some(h) = hash {
        if !verify_password(req.current.as_deref().unwrap_or_default(), &h) {
            return Err(AppError::bad("current password is incorrect"));
        }
    }
    if email.is_none() && phone.is_none() {
        return Err(AppError::bad("add an email or phone number first, so you can sign in with the password"));
    }
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(user.id)
        .bind(hash_password(&req.password)?)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phones() {
        assert_eq!(normalize_phone("+856 20 5555 1234").as_deref(), Some("+8562055551234"));
        assert_eq!(normalize_phone("0066-81-234-5678").as_deref(), Some("+66812345678"));
        assert_eq!(normalize_phone("020 5555 1234"), None); // no country code
        assert_eq!(normalize_phone("+0123456789"), None);
        assert_eq!(normalize_phone("+123"), None);
    }

    #[test]
    fn redirects() {
        assert_eq!(safe_redirect(Some("/dashboard?x=1")), "/dashboard?x=1");
        assert_eq!(safe_redirect(Some("//evil.com")), "/");
        assert_eq!(safe_redirect(Some("/\\evil.com")), "/");
        assert_eq!(safe_redirect(Some("https://evil.com")), "/");
        assert_eq!(safe_redirect(None), "/");
    }

    #[test]
    fn constant_time_eq() {
        assert!(ct_eq("abcd", "abcd"));
        assert!(!ct_eq("abcd", "abce"));
        assert!(!ct_eq("abc", "abcd"));
    }
}
