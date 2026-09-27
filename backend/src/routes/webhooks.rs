//! Inbound webhooks from social platforms.
//!
//! * Meta (Facebook Page comments + Messenger, WhatsApp Cloud API): `GET/POST /api/webhooks/meta`
//!   - GET handshake with `META_VERIFY_TOKEN`; POST bodies verified with `X-Hub-Signature-256`
//!     (HMAC-SHA256 of the raw body with `META_APP_SECRET`).
//! * TikTok: `POST /api/webhooks/tiktok` — `TikTok-Signature: t=<ts>,s=<hmac(ts.body)>` with
//!   `TIKTOK_CLIENT_SECRET`. TikTok does not offer LIVE comments to every app; this endpoint also
//!   accepts the same normalized shape as the generic ingest so relay tools can forward comments.
//! * Generic: `POST /api/webhooks/ingest/{channel_id}` — `X-Zaokaiy-Signature: sha256=<hmac(body)>`
//!   with the channel's secret. For n8n / Make / Zapier / custom bots. The reply text is returned
//!   in the response so the caller can post it back.
//!
//! Signature checks are skipped (with a warning) when the corresponding secret isn't configured,
//! which is convenient in development — always set them in production.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    models::Shop,
    routes::social::{process, Channel, Inbound},
    AppState,
};

type HmacSha256 = Hmac<Sha256>;

#[allow(dead_code)] // handy for tests and for signing outbound test payloads
pub fn sign(secret: &str, data: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

/// Constant-time check of a hex HMAC.
pub fn verify(secret: &str, data: &[u8], hex_sig: &str) -> bool {
    let Ok(sig) = hex::decode(hex_sig.trim()) else { return false };
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(data);
    mac.verify_slice(&sig).is_ok()
}

fn s(v: &Value, keys: &[&str]) -> String {
    for k in keys {
        match &v[*k] {
            Value::String(x) if !x.is_empty() => return x.clone(),
            Value::Number(n) => return n.to_string(),
            _ => {}
        }
    }
    String::new()
}

async fn channel_by_ext(st: &AppState, provider: &str, ext: &str) -> AppResult<Option<(Channel, Shop)>> {
    if ext.is_empty() {
        return Ok(None);
    }
    let c: Option<Channel> = sqlx::query_as("SELECT * FROM social_channels WHERE provider=$1 AND external_id=$2 AND active")
        .bind(provider)
        .bind(ext)
        .fetch_optional(&st.db)
        .await?;
    match c {
        Some(c) => {
            let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id=$1").bind(c.shop_id).fetch_one(&st.db).await?;
            Ok(Some((c, shop)))
        }
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Meta
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct VerifyQ {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    token: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
}

pub async fn meta_verify(State(st): State<AppState>, Query(q): Query<VerifyQ>) -> Response {
    let ok = q.mode.as_deref() == Some("subscribe")
        && !st.cfg.meta_verify_token.is_empty()
        && q.token.as_deref() == Some(st.cfg.meta_verify_token.as_str());
    if ok {
        (StatusCode::OK, q.challenge.unwrap_or_default()).into_response()
    } else {
        (StatusCode::FORBIDDEN, "verification failed").into_response()
    }
}

pub async fn meta_receive(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> AppResult<Json<Value>> {
    if !st.cfg.meta_app_secret.is_empty() {
        let sig = headers
            .get("x-hub-signature-256")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("sha256="))
            .unwrap_or("");
        if !verify(&st.cfg.meta_app_secret, &body, sig) {
            return Err(AppError::Unauthorized);
        }
    } else {
        tracing::warn!("META_APP_SECRET not set — accepting unsigned Meta webhook (dev only)");
    }
    let v: Value = serde_json::from_slice(&body).map_err(|_| AppError::bad("invalid JSON"))?;
    let mut handled = 0;
    let object = v["object"].as_str().unwrap_or_default();
    for entry in v["entry"].as_array().into_iter().flatten() {
        match object {
            "page" => {
                let page_id = s(entry, &["id"]);
                let Some((ch, shop)) = channel_by_ext(&st, "facebook", &page_id).await? else { continue };
                // Page feed: comments on posts and live videos.
                for ch_ in entry["changes"].as_array().into_iter().flatten() {
                    let val = &ch_["value"];
                    if ch_["field"] != "feed" || val["item"] != "comment" || val["verb"] != "add" {
                        continue;
                    }
                    let from_id = s(&val["from"], &["id"]);
                    if from_id == page_id {
                        continue; // our own replies
                    }
                    let msg = s(val, &["message"]);
                    if msg.is_empty() {
                        continue;
                    }
                    process(&st, &shop, Some(&ch), Inbound {
                        provider: "facebook".into(),
                        kind: "comment".into(),
                        external_id: s(val, &["comment_id"]),
                        user_id: from_id,
                        user_name: s(&val["from"], &["name"]),
                        post_id: s(val, &["post_id"]),
                        message: msg,
                        simulate: false,
                    })
                    .await?;
                    handled += 1;
                }
                // Messenger DMs.
                for m in entry["messaging"].as_array().into_iter().flatten() {
                    if m["message"]["is_echo"] == true {
                        continue;
                    }
                    let text = s(&m["message"], &["text"]);
                    if text.is_empty() {
                        continue;
                    }
                    process(&st, &shop, Some(&ch), Inbound {
                        provider: "facebook".into(),
                        kind: "message".into(),
                        external_id: s(&m["message"], &["mid"]),
                        user_id: s(&m["sender"], &["id"]),
                        user_name: String::new(),
                        post_id: String::new(),
                        message: text,
                        simulate: false,
                    })
                    .await?;
                    handled += 1;
                }
            }
            "whatsapp_business_account" => {
                for ch_ in entry["changes"].as_array().into_iter().flatten() {
                    let val = &ch_["value"];
                    let phone_id = s(&val["metadata"], &["phone_number_id"]);
                    let Some((ch, shop)) = channel_by_ext(&st, "whatsapp", &phone_id).await? else { continue };
                    for m in val["messages"].as_array().into_iter().flatten() {
                        let text = match m["type"].as_str() {
                            Some("text") => s(&m["text"], &["body"]),
                            Some("button") => s(&m["button"], &["text"]),
                            _ => String::new(),
                        };
                        if text.is_empty() {
                            continue;
                        }
                        let from = s(m, &["from"]);
                        let name = val["contacts"]
                            .as_array()
                            .and_then(|cs| cs.iter().find(|c| s(c, &["wa_id"]) == from))
                            .map(|c| s(&c["profile"], &["name"]))
                            .unwrap_or_default();
                        process(&st, &shop, Some(&ch), Inbound {
                            provider: "whatsapp".into(),
                            kind: "message".into(),
                            external_id: s(m, &["id"]),
                            user_id: from,
                            user_name: name,
                            post_id: String::new(),
                            message: text,
                            simulate: false,
                        })
                        .await?;
                        handled += 1;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(Json(json!({ "ok": true, "handled": handled })))
}

// ---------------------------------------------------------------------------
// TikTok
// ---------------------------------------------------------------------------

pub async fn tiktok_receive(State(st): State<AppState>, headers: HeaderMap, body: Bytes) -> AppResult<Json<Value>> {
    if !st.cfg.tiktok_client_secret.is_empty() {
        let header = headers.get("tiktok-signature").and_then(|h| h.to_str().ok()).unwrap_or("");
        let (mut t, mut sig) = ("", "");
        for part in header.split(',') {
            if let Some(v) = part.trim().strip_prefix("t=") {
                t = v;
            } else if let Some(v) = part.trim().strip_prefix("s=") {
                sig = v;
            }
        }
        let signed = [t.as_bytes(), b".", &body].concat();
        if t.is_empty() || !verify(&st.cfg.tiktok_client_secret, &signed, sig) {
            return Err(AppError::Unauthorized);
        }
        // Reject replays older than 5 minutes.
        if let Ok(ts) = t.parse::<i64>() {
            if (chrono::Utc::now().timestamp() - ts).abs() > 300 {
                return Err(AppError::Unauthorized);
            }
        }
    } else {
        tracing::warn!("TIKTOK_CLIENT_SECRET not set — accepting unsigned TikTok webhook (dev only)");
    }
    let v: Value = serde_json::from_slice(&body).map_err(|_| AppError::bad("invalid JSON"))?;
    // TikTok wraps event data in a JSON string `content`.
    let content: Value = match &v["content"] {
        Value::String(sv) => serde_json::from_str(sv).unwrap_or(Value::Null),
        other => other.clone(),
    };
    let data = if content.is_object() { &content } else { &v };
    let account = [s(&v, &["user_openid", "account_id", "channel_external_id"]), s(data, &["user_openid", "account_id", "channel_external_id"])]
        .into_iter()
        .find(|x| !x.is_empty())
        .unwrap_or_default();
    let Some((ch, shop)) = channel_by_ext(&st, "tiktok", &account).await? else {
        return Ok(Json(json!({ "ok": true, "handled": 0, "note": "no active TikTok channel for this account" })));
    };
    let user = if data["user"].is_object() { &data["user"] } else { data };
    let text = s(data, &["comment", "text", "content", "message"]);
    if text.is_empty() {
        return Ok(Json(json!({ "ok": true, "handled": 0 })));
    }
    let out = process(&st, &shop, Some(&ch), Inbound {
        provider: "tiktok".into(),
        kind: if s(&v, &["event"]).contains("live") { "live".into() } else { "comment".into() },
        external_id: {
            let id = s(data, &["comment_id", "msg_id", "id"]);
            if id.is_empty() { format!("tt-{}", Uuid::new_v4()) } else { id }
        },
        user_id: s(user, &["open_id", "user_id", "unique_id", "id"]),
        user_name: s(user, &["nickname", "display_name", "user_name", "name"]),
        post_id: s(data, &["video_id", "room_id", "live_id"]),
        message: text,
        simulate: false,
    })
    .await?;
    Ok(Json(json!({ "ok": true, "handled": 1, "result": out.result, "reply_text": out.reply_text, "checkout_url": out.checkout_url })))
}

// ---------------------------------------------------------------------------
// Generic signed ingest (n8n, Make, Zapier, custom bots)
// ---------------------------------------------------------------------------

pub async fn ingest(State(st): State<AppState>, Path(channel_id): Path<Uuid>, headers: HeaderMap, body: Bytes) -> AppResult<Json<Value>> {
    let ch: Channel = sqlx::query_as("SELECT * FROM social_channels WHERE id=$1 AND active")
        .bind(channel_id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let sig = headers
        .get("x-zaokaiy-signature")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("sha256="))
        .unwrap_or("");
    if !verify(&ch.secret, &body, sig) {
        return Err(AppError::Unauthorized);
    }
    let v: Value = serde_json::from_slice(&body).map_err(|_| AppError::bad("invalid JSON"))?;
    let message = s(&v, &["message", "text", "comment"]);
    let user_id = s(&v, &["user_id", "from", "sender_id"]);
    if message.is_empty() || user_id.is_empty() {
        return Err(AppError::bad("`message` and `user_id` are required"));
    }
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id=$1").bind(ch.shop_id).fetch_one(&st.db).await?;
    let provider = match s(&v, &["provider"]).as_str() {
        p @ ("facebook" | "tiktok" | "whatsapp") => p.to_string(),
        _ => ch.provider.clone(),
    };
    let out = process(&st, &shop, Some(&ch), Inbound {
        provider,
        kind: match s(&v, &["kind"]).as_str() {
            k @ ("comment" | "message" | "live") => k.to_string(),
            _ => "comment".into(),
        },
        external_id: {
            let id = s(&v, &["id", "comment_id", "message_id"]);
            if id.is_empty() { format!("in-{}", Uuid::new_v4()) } else { id }
        },
        user_id,
        user_name: s(&v, &["user_name", "name", "nickname"]),
        post_id: s(&v, &["post_id", "live_id"]),
        message,
        simulate: false,
    })
    .await?;
    Ok(Json(serde_json::to_value(out).unwrap()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hmac_roundtrip() {
        let sig = sign("secret", b"{\"a\":1}");
        assert!(verify("secret", b"{\"a\":1}", &sig));
        assert!(!verify("secret", b"{\"a\":2}", &sig));
        assert!(!verify("other", b"{\"a\":1}", &sig));
        assert!(!verify("secret", b"x", "zz-not-hex"));
    }
}
