//! Cloudflare Turnstile human check for sign-in, sign-up and WhatsApp code requests.
//!
//! The widget in the browser produces a single-use token (valid 5 minutes) that is sent as
//! `captcha` in the request body and checked here with Cloudflare's siteverify API. Disabled
//! when `TURNSTILE_SECRET_KEY` / `TURNSTILE_SITE_KEY` are empty (local development).

use axum::http::HeaderMap;
use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    AppState,
};

const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

pub fn enabled(st: &AppState) -> bool {
    !st.cfg.turnstile_secret_key.is_empty() && !st.cfg.turnstile_site_key.is_empty()
}

/// Visitor IP as Cloudflare reported it (the API sits behind the Cloudflare tunnel).
fn client_ip(headers: &HeaderMap) -> Option<String> {
    headers.get("cf-connecting-ip")?.to_str().ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub async fn verify(st: &AppState, headers: &HeaderMap, token: Option<&str>) -> AppResult<()> {
    if !enabled(st) {
        return Ok(());
    }
    let token = token.map(str::trim).filter(|t| !t.is_empty() && t.len() <= 2048).ok_or_else(|| AppError::bad("please complete the human verification"))?;
    let mut body = json!({ "secret": st.cfg.turnstile_secret_key, "response": token });
    if let Some(ip) = client_ip(headers) {
        body["remoteip"] = json!(ip);
    }
    let res = st
        .http
        .post(SITEVERIFY_URL)
        .timeout(std::time::Duration::from_secs(10))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "turnstile siteverify unreachable");
            AppError::Upstream("could not check the human verification, please try again".into())
        })?;
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if v["success"].as_bool() == Some(true) {
        Ok(())
    } else {
        tracing::info!(errors = %v["error-codes"], "turnstile rejected a token");
        Err(AppError::bad("human verification failed, please try again"))
    }
}
