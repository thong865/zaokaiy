//! Social commerce: turn comments / chat messages into orders.
//!
//! Flow: inbound comment (webhook or simulator) → dedupe → parse codes → reserve stock on the
//! customer's open social order → reply with a checkout link → customer confirms shipping details
//! → seller verifies payment → ships. Unconfirmed/unpaid orders expire and release their stock.

use std::collections::BTreeMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::Shop,
    routes::{logistics, owned_shop},
    social_parser, AppState,
};

pub const PROVIDERS: [&str; 4] = ["facebook", "tiktok", "whatsapp", "webhook"];

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct Channel {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub provider: String,
    pub name: String,
    pub external_id: String,
    #[serde(skip)]
    pub access_token: String,
    pub secret: String,
    pub active: bool,
    pub auto_reply: bool,
    pub last_event_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Channel {
    fn public(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap();
        v["has_token"] = json!(!self.access_token.is_empty());
        v
    }
}

#[derive(Debug, Serialize, FromRow)]
pub struct SocialOrder {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub customer_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub number: String,
    pub status: String,
    #[serde(skip)]
    pub token: String,
    pub subtotal_cents: i64,
    pub shipping_cents: i64,
    pub total_cents: i64,
    pub currency: String,
    pub ship_name: String,
    pub ship_phone: String,
    pub ship_address: String,
    pub customer_note: String,
    pub payment_method: String,
    pub payment_ref: String,
    pub tracking_no: String,
    pub expires_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub carrier_code: Option<String>,
    pub delivery_type: String,
    pub fee_payer: String,
    pub cod_fee_cents: i64,
    pub cod_amount_cents: i64,
    pub cod_status: String,
    pub cod_remit_ref: String,
    pub cod_collected_at: Option<DateTime<Utc>>,
    pub cod_remitted_at: Option<DateTime<Utc>>,
    pub shipped_at: Option<DateTime<Utc>>,
    /// Coupon + points taken off (module `promo`); `total_cents` is already net of it.
    pub discount_cents: i64,
    pub platform_discount_cents: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct SocialItem {
    pub id: Uuid,
    pub product_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub qty: i32,
    pub unit_price_cents: i64,
}

/// A normalized inbound comment/message from any provider.
#[derive(Debug, Clone)]
pub struct Inbound {
    pub provider: String,
    pub kind: String, // comment | message | live
    pub external_id: String,
    pub user_id: String,
    pub user_name: String,
    pub post_id: String,
    pub message: String,
    /// Don't call external APIs for the reply (simulator).
    pub simulate: bool,
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    pub comment_id: Uuid,
    pub duplicate: bool,
    pub result: String,
    pub order_id: Option<Uuid>,
    pub order_number: Option<String>,
    pub checkout_url: Option<String>,
    pub reply_text: String,
    pub reply_status: String,
    pub claims: Vec<Value>,
}

fn token() -> String {
    rand::thread_rng().sample_iter(&Alphanumeric).take(32).map(char::from).collect()
}

pub fn new_secret() -> String {
    format!("zk_{}", token())
}

fn money(cents: i64, cur: &str) -> String {
    let sym = match cur {
        "THB" => "฿",
        "USD" => "$",
        "LAK" => "₭",
        _ => "",
    };
    if cents % 100 == 0 {
        format!("{sym}{}", group(cents / 100))
    } else {
        format!("{sym}{}.{:02}", group(cents / 100), cents % 100)
    }
}

fn group(n: i64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn render(template: &str, vars: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

async fn next_number(tx: &mut Transaction<'_, Postgres>, shop_id: Uuid) -> AppResult<String> {
    let n: i64 = sqlx::query_scalar(
        "INSERT INTO shop_counters (shop_id, kind, next) VALUES ($1, 'social', 2)
         ON CONFLICT (shop_id, kind) DO UPDATE SET next = shop_counters.next + 1 RETURNING next - 1",
    )
    .bind(shop_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(format!("SO{n:06}"))
}

async fn move_stock(
    tx: &mut Transaction<'_, Postgres>,
    product_id: Uuid,
    delta: i32,
    reason: &str,
    order_id: Uuid,
    note: &str,
) -> AppResult<()> {
    let after: i32 = sqlx::query_scalar("UPDATE products SET stock = stock + $2, updated_at = now() WHERE id = $1 RETURNING stock")
        .bind(product_id)
        .bind(delta)
        .fetch_one(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO inventory_movements (product_id, delta, stock_after, reason, ref_social_order_id, note)
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(product_id)
    .bind(delta)
    .bind(after)
    .bind(reason)
    .bind(order_id)
    .bind(note)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn recalc(tx: &mut Transaction<'_, Postgres>, order_id: Uuid, shipping_flat: i64) -> AppResult<(i64, i64)> {
    let subtotal: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(qty * unit_price_cents), 0)::bigint FROM social_order_items WHERE order_id = $1",
    )
    .bind(order_id)
    .fetch_one(&mut **tx)
    .await?;
    // With a courier chosen at checkout, its terms decide the fee (and COD fee); otherwise the flat social fee.
    let (shop_id, carrier, delivery, method): (Uuid, Option<String>, String, String) =
        sqlx::query_as("SELECT shop_id, carrier_code, delivery_type, payment_method FROM social_orders WHERE id=$1")
            .bind(order_id)
            .fetch_one(&mut **tx)
            .await?;
    let quoted = match (&carrier, subtotal > 0) {
        (Some(c), true) => logistics::quote(&mut **tx, shop_id, c, &delivery, method == "cod", subtotal).await.ok(),
        _ => None,
    };
    let (shipping, cod_fee, payer, cod) = match &quoted {
        Some(q) => (q.charged_fee_cents, q.cod_fee_cents, q.fee_payer.clone(), q.cod),
        None => (if subtotal > 0 && carrier.is_none() { shipping_flat } else { 0 }, 0, "buyer".to_string(), false),
    };
    // A coupon/points discount (module promo) never takes more than the goods + shipping charged.
    let discount: i64 = sqlx::query_scalar("SELECT discount_cents FROM social_orders WHERE id=$1").bind(order_id).fetch_one(&mut **tx).await?;
    let total = subtotal + shipping + cod_fee - discount.min(subtotal + shipping);
    sqlx::query(
        "UPDATE social_orders SET subtotal_cents=$2, shipping_cents=$3, total_cents=$4, cod_fee_cents=$5, fee_payer=$6,
            cod_amount_cents = CASE WHEN $7 THEN $4 ELSE 0 END, updated_at=now() WHERE id=$1",
    )
    .bind(order_id)
    .bind(subtotal)
    .bind(shipping)
    .bind(total)
    .bind(cod_fee)
    .bind(payer)
    .bind(cod)
    .execute(&mut **tx)
    .await?;
    Ok((subtotal, total))
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

pub async fn process(st: &AppState, shop: &Shop, channel: Option<&Channel>, inb: Inbound) -> AppResult<Outcome> {
    let session_id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM social_sessions WHERE shop_id = $1 AND status = 'live'")
            .bind(shop.id)
            .fetch_optional(&st.db)
            .await?;
    let message: String = inb.message.chars().take(2000).collect();

    // 1. Deduplicate (webhooks are retried).
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO social_comments (shop_id, channel_id, session_id, provider, kind, external_id, external_user_id,
                                      user_name, post_id, message)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         ON CONFLICT (shop_id, provider, external_id) DO NOTHING RETURNING id",
    )
    .bind(shop.id)
    .bind(channel.map(|c| c.id))
    .bind(session_id)
    .bind(&inb.provider)
    .bind(&inb.kind)
    .bind(&inb.external_id)
    .bind(&inb.user_id)
    .bind(&inb.user_name)
    .bind(&inb.post_id)
    .bind(&message)
    .fetch_optional(&st.db)
    .await?;
    let Some(comment_id) = inserted else {
        let existing: (Uuid, String, Option<Uuid>, String, String) = sqlx::query_as(
            "SELECT id, result, order_id, reply_text, reply_status FROM social_comments
             WHERE shop_id=$1 AND provider=$2 AND external_id=$3",
        )
        .bind(shop.id)
        .bind(&inb.provider)
        .bind(&inb.external_id)
        .fetch_one(&st.db)
        .await?;
        return Ok(Outcome {
            comment_id: existing.0, duplicate: true, result: existing.1, order_id: existing.2, order_number: None,
            checkout_url: None, reply_text: existing.3, reply_status: existing.4, claims: vec![],
        });
    };
    if let Some(c) = channel {
        sqlx::query("UPDATE social_channels SET last_event_at = now() WHERE id = $1").bind(c.id).execute(&st.db).await?;
    }

    // 2. Parse against this shop's product codes.
    let catalog: Vec<(Uuid, String, String, i64, i32)> = sqlx::query_as(
        "SELECT id, upper(social_code), name, price_cents, stock FROM products
         WHERE shop_id = $1 AND social_code IS NOT NULL AND social_code <> '' AND status <> 'archived'",
    )
    .bind(shop.id)
    .fetch_all(&st.db)
    .await?;
    let codes: Vec<String> = catalog.iter().map(|c| c.1.clone()).collect();
    let parsed = social_parser::parse(&message, &codes, &shop.social_triggers, shop.social_require_trigger, shop.social_max_qty);
    let parsed_json = json!(parsed.claims.iter().map(|c| json!({ "code": c.code, "qty": c.qty })).collect::<Vec<_>>());

    let finish = |result: &str| (result.to_string(), comment_id);
    if parsed.claims.is_empty() {
        sqlx::query("UPDATE social_comments SET parsed=$2, result='ignored' WHERE id=$1")
            .bind(comment_id)
            .bind(&parsed_json)
            .execute(&st.db)
            .await?;
        let (result, _) = finish("ignored");
        return Ok(Outcome { comment_id, duplicate: false, result, order_id: None, order_number: None, checkout_url: None,
                            reply_text: String::new(), reply_status: "none".into(), claims: vec![] });
    }

    // 3. Customer (blocked customers are ignored).
    let (customer_id, blocked): (Uuid, bool) = sqlx::query_as(
        "INSERT INTO social_customers (shop_id, provider, external_user_id, name) VALUES ($1,$2,$3,$4)
         ON CONFLICT (shop_id, provider, external_user_id) DO UPDATE
           SET name = CASE WHEN EXCLUDED.name <> '' THEN EXCLUDED.name ELSE social_customers.name END
         RETURNING id, blocked",
    )
    .bind(shop.id)
    .bind(&inb.provider)
    .bind(&inb.user_id)
    .bind(&inb.user_name)
    .fetch_one(&st.db)
    .await?;
    if blocked {
        sqlx::query("UPDATE social_comments SET parsed=$2, result='blocked' WHERE id=$1")
            .bind(comment_id)
            .bind(&parsed_json)
            .execute(&st.db)
            .await?;
        return Ok(Outcome { comment_id, duplicate: false, result: "blocked".into(), order_id: None, order_number: None,
                            checkout_url: None, reply_text: String::new(), reply_status: "none".into(), claims: vec![] });
    }

    // 4. Reserve stock on the customer's open order (first come, first served).
    let mut tx = st.db.begin().await?;
    let mut ids: Vec<Uuid> = parsed
        .claims
        .iter()
        .filter_map(|c| catalog.iter().find(|p| p.1 == c.code).map(|p| p.0))
        .collect();
    ids.sort();
    let locked: Vec<(Uuid, String, String, i64, i32)> = sqlx::query_as(
        "SELECT id, upper(social_code), name, price_cents, stock FROM products WHERE id = ANY($1) ORDER BY id FOR UPDATE",
    )
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    let by_code: BTreeMap<String, (Uuid, String, i64, i32)> =
        locked.into_iter().map(|(id, code, name, price, stock)| (code, (id, name, price, stock))).collect();

    let open: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, number, token FROM social_orders WHERE customer_id = $1 AND status = 'open' FOR UPDATE",
    )
    .bind(customer_id)
    .fetch_optional(&mut *tx)
    .await?;
    let expires = Utc::now() + Duration::hours(shop.social_hold_hours as i64);
    let (order_id, number, tok) = match open {
        Some(o) => o,
        None => {
            let number = next_number(&mut tx, shop.id).await?;
            let tok = token();
            let id: Uuid = sqlx::query_scalar(
                "INSERT INTO social_orders (shop_id, customer_id, channel_id, session_id, number, token, currency, expires_at, ship_name)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id",
            )
            .bind(shop.id)
            .bind(customer_id)
            .bind(channel.map(|c| c.id))
            .bind(session_id)
            .bind(&number)
            .bind(&tok)
            .bind(&shop.currency)
            .bind(expires)
            .bind(&inb.user_name)
            .fetch_one(&mut *tx)
            .await?;
            (id, number, tok)
        }
    };

    let mut got: Vec<String> = Vec::new();
    let mut missed: Vec<String> = Vec::new();
    let mut claims_out: Vec<Value> = Vec::new();
    for c in &parsed.claims {
        let Some((pid, name, price, stock)) = by_code.get(&c.code) else { continue };
        let already: i32 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(qty),0)::int FROM social_order_items WHERE order_id=$1 AND product_id=$2",
        )
        .bind(order_id)
        .bind(pid)
        .fetch_one(&mut *tx)
        .await?;
        let room = (shop.social_max_qty - already).max(0);
        let want = c.qty.min(room);
        let take = want.min(*stock).max(0);
        if take > 0 {
            move_stock(&mut tx, *pid, -take, "reserve", order_id, &format!("Social {number}")).await?;
            sqlx::query(
                "INSERT INTO social_order_items (order_id, product_id, code, name, qty, unit_price_cents)
                 VALUES ($1,$2,$3,$4,$5,$6)
                 ON CONFLICT (order_id, product_id) DO UPDATE SET qty = social_order_items.qty + EXCLUDED.qty",
            )
            .bind(order_id)
            .bind(pid)
            .bind(&c.code)
            .bind(name)
            .bind(take)
            .bind(price)
            .execute(&mut *tx)
            .await?;
            got.push(format!("{} {} x{}", c.code, name, take));
        }
        if take < c.qty {
            missed.push(if take == 0 { format!("{} {}", c.code, name) } else { format!("{} {} ({} more)", c.code, name, c.qty - take) });
        }
        claims_out.push(json!({ "code": c.code, "name": name, "requested": c.qty, "reserved": take }));
    }

    let (_, total) = recalc(&mut tx, order_id, shop.social_shipping_cents).await?;
    let result = match (got.is_empty(), missed.is_empty()) {
        (false, true) => "claimed",
        (false, false) => "partial",
        _ => "sold_out",
    };
    if !got.is_empty() {
        sqlx::query("UPDATE social_orders SET expires_at=$2, session_id=COALESCE(session_id,$3), channel_id=COALESCE(channel_id,$4) WHERE id=$1")
            .bind(order_id)
            .bind(expires)
            .bind(session_id)
            .bind(channel.map(|c| c.id))
            .execute(&mut *tx)
            .await?;
    }
    // Drop an empty order we just created for a fully sold-out comment.
    let keep_order = total > 0;
    if !keep_order {
        sqlx::query("DELETE FROM social_orders WHERE id=$1 AND subtotal_cents=0").bind(order_id).execute(&mut *tx).await?;
    }
    let link = format!("{}/c/{}", st.cfg.public_web_url.trim_end_matches('/'), tok);
    let name = if inb.user_name.is_empty() { "คุณลูกค้า".to_string() } else { inb.user_name.clone() };
    let mut reply = String::new();
    if !got.is_empty() {
        reply = render(&shop.social_reply_template, &[
            ("name", name.clone()), ("items", got.join(", ")), ("total", money(total, &shop.currency)),
            ("link", link.clone()), ("hours", shop.social_hold_hours.to_string()), ("order", number.clone()),
        ]);
    }
    if !missed.is_empty() {
        let s = render(&shop.social_soldout_template, &[("name", name), ("items", missed.join(", "))]);
        reply = if reply.is_empty() { s } else { format!("{reply}\n{s}") };
    }
    sqlx::query("UPDATE social_comments SET parsed=$2, result=$3, order_id=$4, reply_text=$5 WHERE id=$1")
        .bind(comment_id)
        .bind(&parsed_json)
        .bind(result)
        .bind(keep_order.then_some(order_id))
        .bind(&reply)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // 5. Reply on the originating platform.
    let reply_status = send_reply(st, channel, &inb, comment_id, &reply).await;

    Ok(Outcome {
        comment_id,
        duplicate: false,
        result: result.into(),
        order_id: keep_order.then_some(order_id),
        order_number: keep_order.then_some(number),
        checkout_url: keep_order.then_some(link),
        reply_text: reply,
        reply_status,
        claims: claims_out,
    })
}

/// Sends the reply through the platform API when possible. Returns the stored reply status.
async fn send_reply(st: &AppState, channel: Option<&Channel>, inb: &Inbound, comment_id: Uuid, text: &str) -> String {
    if text.is_empty() {
        return "none".into();
    }
    let (status, err) = match channel {
        _ if inb.simulate => ("simulated", String::new()),
        None => ("simulated", String::new()),
        Some(c) if !c.auto_reply => ("none", String::new()),
        Some(c) if c.access_token.is_empty() && matches!(c.provider.as_str(), "facebook" | "whatsapp") => {
            ("failed", "no access token configured for this channel".to_string())
        }
        Some(c) => match (c.provider.as_str(), inb.kind.as_str()) {
            ("facebook", "comment") => {
                // Private reply (DM) with the details + short public acknowledgement.
                let dm = graph_post(st, &format!("{}/messages", c.external_id), &c.access_token,
                    json!({ "recipient": { "comment_id": inb.external_id }, "message": { "text": text } })).await;
                let _ = graph_post(st, &format!("{}/comments", inb.external_id), &c.access_token,
                    json!({ "message": "✅ ส่งรายละเอียดทางแชทแล้วค่ะ / Details sent via Messenger" })).await;
                match dm { Ok(_) => ("sent", String::new()), Err(e) => ("failed", e) }
            }
            ("facebook", _) => match graph_post(st, "me/messages", &c.access_token,
                json!({ "recipient": { "id": inb.user_id }, "messaging_type": "RESPONSE", "message": { "text": text } })).await {
                Ok(_) => ("sent", String::new()),
                Err(e) => ("failed", e),
            },
            ("whatsapp", _) => match graph_post(st, &format!("{}/messages", c.external_id), &c.access_token,
                json!({ "messaging_product": "whatsapp", "to": inb.user_id, "type": "text", "text": { "preview_url": true, "body": text } })).await {
                Ok(_) => ("sent", String::new()),
                Err(e) => ("failed", e),
            },
            // TikTok has no public API to reply to LIVE comments; generic webhooks get the reply in the HTTP response.
            _ => ("unsupported", String::new()),
        },
    };
    let _ = sqlx::query("UPDATE social_comments SET reply_status=$2, reply_error=$3 WHERE id=$1")
        .bind(comment_id)
        .bind(status)
        .bind(&err)
        .execute(&st.db)
        .await;
    status.into()
}

async fn graph_post(st: &AppState, path: &str, token: &str, body: Value) -> Result<Value, String> {
    let url = format!("{}/{}", st.cfg.meta_graph_url.trim_end_matches('/'), path);
    let res = st.http.post(&url).bearer_auth(token).json(&body).send().await.map_err(|e| e.to_string())?;
    let status = res.status();
    let v: Value = res.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        Ok(v)
    } else {
        Err(v["error"]["message"].as_str().map(String::from).unwrap_or_else(|| format!("HTTP {status}")))
    }
}

/// Release stock of an order being cancelled/expired (inside `tx`).
async fn release(tx: &mut Transaction<'_, Postgres>, order_id: Uuid, number: &str, why: &str) -> AppResult<()> {
    let items: Vec<(Uuid, i32)> = sqlx::query_as(
        "SELECT product_id, qty FROM social_order_items WHERE order_id=$1 AND product_id IS NOT NULL",
    )
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;
    for (pid, qty) in items {
        move_stock(tx, pid, qty, "release", order_id, &format!("{why} {number}")).await?;
    }
    // Module hook (promo): coupon and points go back to the customer.
    crate::modules::promo::sale_cancelled(&mut **tx, "social", order_id, None).await?;
    Ok(())
}

/// Background job: expire unconfirmed/unpaid orders and return their stock.
pub async fn expire_due(st: &AppState) -> AppResult<u64> {
    let due: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, number FROM social_orders WHERE status IN ('open','confirmed') AND expires_at < now() LIMIT 200",
    )
    .fetch_all(&st.db)
    .await?;
    let mut n = 0;
    for (id, number) in due {
        let mut tx = st.db.begin().await?;
        let ok: Option<Uuid> = sqlx::query_scalar(
            "UPDATE social_orders SET status='expired', updated_at=now()
             WHERE id=$1 AND status IN ('open','confirmed') AND expires_at < now() RETURNING id",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if ok.is_some() {
            release(&mut tx, id, &number, "Expired").await?;
            n += 1;
        }
        tx.commit().await?;
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Seller endpoints
// ---------------------------------------------------------------------------

pub async fn settings(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    let s = owned_shop(&st, shop_id, &user).await?;
    Ok(Json(settings_json(&s, &st)))
}

fn settings_json(s: &Shop, st: &AppState) -> Value {
    json!({
        "triggers": s.social_triggers, "require_trigger": s.social_require_trigger, "hold_hours": s.social_hold_hours,
        "max_qty": s.social_max_qty, "shipping_cents": s.social_shipping_cents, "reply_template": s.social_reply_template,
        "soldout_template": s.social_soldout_template, "payment_instructions": s.payment_instructions,
        "meta_webhook_configured": !st.cfg.meta_verify_token.is_empty(),
        "meta_signature_check": !st.cfg.meta_app_secret.is_empty(),
        "tiktok_signature_check": !st.cfg.tiktok_client_secret.is_empty(),
    })
}

#[derive(Deserialize)]
pub struct SettingsReq {
    pub triggers: Option<Vec<String>>,
    pub require_trigger: Option<bool>,
    pub hold_hours: Option<i32>,
    pub max_qty: Option<i32>,
    pub shipping_cents: Option<i64>,
    pub reply_template: Option<String>,
    pub soldout_template: Option<String>,
    pub payment_instructions: Option<String>,
}

pub async fn update_settings(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(r): Json<SettingsReq>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    if matches!(r.hold_hours, Some(h) if !(1..=336).contains(&h)) {
        return Err(AppError::bad("hold time must be 1–336 hours"));
    }
    if matches!(r.max_qty, Some(q) if !(1..=999).contains(&q)) {
        return Err(AppError::bad("max quantity must be 1–999"));
    }
    if matches!(r.shipping_cents, Some(c) if c < 0) {
        return Err(AppError::bad("shipping cannot be negative"));
    }
    let triggers = r.triggers.map(|t| {
        t.into_iter().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty() && s.chars().count() <= 20).take(30).collect::<Vec<_>>()
    });
    let s: Shop = sqlx::query_as(
        "UPDATE shops SET
            social_triggers = COALESCE($2, social_triggers),
            social_require_trigger = COALESCE($3, social_require_trigger),
            social_hold_hours = COALESCE($4, social_hold_hours),
            social_max_qty = COALESCE($5, social_max_qty),
            social_shipping_cents = COALESCE($6, social_shipping_cents),
            social_reply_template = COALESCE($7, social_reply_template),
            social_soldout_template = COALESCE($8, social_soldout_template),
            payment_instructions = COALESCE($9, payment_instructions)
         WHERE id = $1 RETURNING *",
    )
    .bind(shop_id)
    .bind(triggers)
    .bind(r.require_trigger)
    .bind(r.hold_hours)
    .bind(r.max_qty)
    .bind(r.shipping_cents)
    .bind(r.reply_template.map(|t| t.chars().take(1000).collect::<String>()))
    .bind(r.soldout_template.map(|t| t.chars().take(500).collect::<String>()))
    .bind(r.payment_instructions.map(|t| t.chars().take(2000).collect::<String>()))
    .fetch_one(&st.db)
    .await?;
    Ok(Json(settings_json(&s, &st)))
}

// ---- channels ----

pub async fn list_channels(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows: Vec<Channel> = sqlx::query_as("SELECT * FROM social_channels WHERE shop_id=$1 ORDER BY created_at")
        .bind(shop_id)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(rows.iter().map(Channel::public).collect()))
}

#[derive(Deserialize)]
pub struct ChannelReq {
    pub provider: Option<String>,
    pub name: Option<String>,
    pub external_id: Option<String>,
    /// Write-only.
    pub access_token: Option<String>,
    pub active: Option<bool>,
    pub auto_reply: Option<bool>,
    pub rotate_secret: Option<bool>,
}

pub async fn create_channel(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(r): Json<ChannelReq>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let provider = r.provider.unwrap_or_default();
    if !PROVIDERS.contains(&provider.as_str()) {
        return Err(AppError::bad(format!("provider must be one of {PROVIDERS:?}")));
    }
    let external_id = r.external_id.unwrap_or_default().trim().to_string();
    if provider != "webhook" && external_id.is_empty() {
        return Err(AppError::bad(match provider.as_str() {
            "facebook" => "enter the Facebook Page ID",
            "whatsapp" => "enter the WhatsApp phone number ID",
            _ => "enter the TikTok account open_id",
        }));
    }
    let c: Channel = sqlx::query_as(
        "INSERT INTO social_channels (shop_id, provider, name, external_id, access_token, secret, auto_reply)
         VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING *",
    )
    .bind(shop_id)
    .bind(&provider)
    .bind(r.name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| provider.clone()))
    .bind(&external_id)
    .bind(r.access_token.unwrap_or_default().trim())
    .bind(new_secret())
    .bind(r.auto_reply.unwrap_or(true))
    .fetch_one(&st.db)
    .await
    .map_err(|e| match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict("that account is already connected (possibly by another shop)".into()),
        o => o,
    })?;
    Ok(Json(c.public()))
}

async fn owned_channel(st: &AppState, id: Uuid, user: &AuthUser) -> AppResult<Channel> {
    let c: Channel = sqlx::query_as("SELECT * FROM social_channels WHERE id=$1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(st, c.shop_id, user).await?;
    Ok(c)
}

pub async fn update_channel(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<ChannelReq>) -> AppResult<Json<Value>> {
    owned_channel(&st, id, &user).await?;
    let c: Channel = sqlx::query_as(
        "UPDATE social_channels SET name=COALESCE($2,name), external_id=COALESCE($3,external_id),
            access_token = CASE WHEN $4::text IS NULL THEN access_token ELSE $4 END,
            active=COALESCE($5,active), auto_reply=COALESCE($6,auto_reply),
            secret = CASE WHEN $7 THEN $8 ELSE secret END
         WHERE id=$1 RETURNING *",
    )
    .bind(id)
    .bind(r.name)
    .bind(r.external_id.map(|s| s.trim().to_string()))
    .bind(r.access_token.map(|s| s.trim().to_string()))
    .bind(r.active)
    .bind(r.auto_reply)
    .bind(r.rotate_secret.unwrap_or(false))
    .bind(new_secret())
    .fetch_one(&st.db)
    .await?;
    Ok(Json(c.public()))
}

pub async fn delete_channel(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_channel(&st, id, &user).await?;
    sqlx::query("DELETE FROM social_channels WHERE id=$1").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

// ---- sessions ----

#[derive(Deserialize)]
pub struct SessionReq {
    pub title: String,
}

pub async fn start_session(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<SessionReq>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    sqlx::query("UPDATE social_sessions SET status='ended', ended_at=now() WHERE shop_id=$1 AND status='live'")
        .bind(shop_id)
        .execute(&st.db)
        .await?;
    let title = r.title.trim();
    let row: (Uuid, String, DateTime<Utc>) = sqlx::query_as(
        "INSERT INTO social_sessions (shop_id, title) VALUES ($1,$2) RETURNING id, title, started_at",
    )
    .bind(shop_id)
    .bind(if title.is_empty() { "Live" } else { title })
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({ "id": row.0, "title": row.1, "status": "live", "started_at": row.2 })))
}

pub async fn end_session(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let shop_id: Uuid = sqlx::query_scalar("SELECT shop_id FROM social_sessions WHERE id=$1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(&st, shop_id, &user).await?;
    sqlx::query("UPDATE social_sessions SET status='ended', ended_at=now() WHERE id=$1 AND status='live'")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn sessions(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows: Vec<(Uuid, String, String, DateTime<Utc>, Option<DateTime<Utc>>, i64, i64)> = sqlx::query_as(
        "SELECT s.id, s.title, s.status, s.started_at, s.ended_at,
                (SELECT COUNT(*) FROM social_comments c WHERE c.session_id = s.id),
                (SELECT COALESCE(SUM(total_cents),0)::bigint FROM social_orders o WHERE o.session_id = s.id AND o.status NOT IN ('cancelled','expired'))
         FROM social_sessions s WHERE s.shop_id=$1 ORDER BY s.started_at DESC LIMIT 50",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows.into_iter().map(|(id, t, s, a, e, c, r)| json!({
        "id": id, "title": t, "status": s, "started_at": a, "ended_at": e, "comments": c, "revenue_cents": r
    })).collect()))
}

// ---- live board ----

#[derive(Deserialize)]
pub struct FeedQ {
    pub session_id: Option<Uuid>,
    pub since: Option<DateTime<Utc>>,
    pub only_orders: Option<bool>,
    pub limit: Option<i64>,
}

pub async fn feed(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Query(q): Query<FeedQ>) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, shop_id, &user).await?;
    type Row = (Uuid, String, String, String, String, String, Value, String, Option<String>, String, String, String, DateTime<Utc>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT c.id, c.provider, c.kind, c.user_name, c.external_user_id, c.message, c.parsed, c.result,
                o.number, c.reply_text, c.reply_status, c.reply_error, c.created_at
         FROM social_comments c LEFT JOIN social_orders o ON o.id = c.order_id
         WHERE c.shop_id = $1
           AND ($2::uuid IS NULL OR c.session_id = $2)
           AND ($3::timestamptz IS NULL OR c.created_at > $3)
           AND (NOT $4 OR c.result <> 'ignored')
         ORDER BY c.created_at DESC LIMIT $5",
    )
    .bind(shop_id)
    .bind(q.session_id)
    .bind(q.since)
    .bind(q.only_orders.unwrap_or(false))
    .bind(q.limit.unwrap_or(100).clamp(1, 500))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows.into_iter().map(|(id, provider, kind, name, uid, msg, parsed, result, number, reply, rs, re, at)| json!({
        "id": id, "provider": provider, "kind": kind, "user_name": name, "user_id": uid, "message": msg, "parsed": parsed,
        "result": result, "order_number": number, "reply_text": reply, "reply_status": rs, "reply_error": re, "created_at": at
    })).collect()))
}

#[derive(Deserialize)]
pub struct BoardQ {
    pub session_id: Option<Uuid>,
}

/// Products with codes, how many were claimed (in the session, or in open/active orders) and stock left.
pub async fn board(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Query(q): Query<BoardQ>) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    let live: Option<(Uuid, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, title, started_at FROM social_sessions WHERE shop_id=$1 AND status='live'",
    )
    .bind(shop_id)
    .fetch_optional(&st.db)
    .await?;
    let session = q.session_id.or(live.as_ref().map(|l| l.0));
    let products: Vec<(Uuid, String, String, i64, i32, Value, i64, i64)> = sqlx::query_as(
        "SELECT p.id, p.social_code, p.name, p.price_cents, p.stock, p.images,
                COALESCE(SUM(i.qty) FILTER (WHERE o.status NOT IN ('cancelled','expired')), 0)::bigint,
                COUNT(DISTINCT o.customer_id) FILTER (WHERE o.status NOT IN ('cancelled','expired'))
         FROM products p
         LEFT JOIN social_order_items i ON i.product_id = p.id
         LEFT JOIN social_orders o ON o.id = i.order_id AND ($2::uuid IS NULL OR o.session_id = $2)
         WHERE p.shop_id = $1 AND p.social_code IS NOT NULL AND p.social_code <> '' AND p.status <> 'archived'
         GROUP BY p.id ORDER BY p.social_code",
    )
    .bind(shop_id)
    .bind(session)
    .fetch_all(&st.db)
    .await?;
    let (orders, revenue, comments, buyers): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE o.status NOT IN ('cancelled','expired')),
                COALESCE(SUM(o.total_cents) FILTER (WHERE o.status NOT IN ('cancelled','expired')),0)::bigint,
                (SELECT COUNT(*) FROM social_comments c WHERE c.shop_id=$1 AND ($2::uuid IS NULL OR c.session_id=$2)),
                COUNT(DISTINCT o.customer_id) FILTER (WHERE o.status NOT IN ('cancelled','expired'))
         FROM social_orders o WHERE o.shop_id=$1 AND ($2::uuid IS NULL OR o.session_id=$2)",
    )
    .bind(shop_id)
    .bind(session)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({
        "session": live.map(|(id, title, at)| json!({ "id": id, "title": title, "started_at": at })),
        "currency": shop.currency,
        "totals": { "orders": orders, "revenue_cents": revenue, "comments": comments, "buyers": buyers },
        "products": products.into_iter().map(|(id, code, name, price, stock, images, claimed, buyers)| json!({
            "id": id, "code": code, "name": name, "price_cents": price, "stock": stock, "image": images.get(0),
            "claimed": claimed, "buyers": buyers
        })).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
pub struct SimulateReq {
    pub provider: Option<String>,
    pub user_name: String,
    pub user_id: Option<String>,
    pub message: String,
}

/// Test the whole pipeline without connecting a real account.
pub async fn simulate(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<SimulateReq>) -> AppResult<Json<Outcome>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    let provider = r.provider.filter(|p| PROVIDERS.contains(&p.as_str())).unwrap_or_else(|| "facebook".into());
    if r.message.trim().is_empty() {
        return Err(AppError::bad("type a comment"));
    }
    let name = r.user_name.trim().to_string();
    let uid = r.user_id.filter(|u| !u.is_empty()).unwrap_or_else(|| format!("sim-{}", name.to_lowercase().replace(' ', "-")));
    let out = process(&st, &shop, None, Inbound {
        provider,
        kind: "live".into(),
        external_id: format!("sim-{}", Uuid::new_v4()),
        user_id: uid,
        user_name: name,
        post_id: String::new(),
        message: r.message,
        simulate: true,
    })
    .await?;
    Ok(Json(out))
}

// ---- orders ----

#[derive(Deserialize)]
pub struct OrdersQ {
    pub status: Option<String>,
    pub q: Option<String>,
    pub session_id: Option<Uuid>,
    pub limit: Option<i64>,
}

pub async fn list_orders(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Query(q): Query<OrdersQ>) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, shop_id, &user).await?;
    type Row = (Uuid, String, String, i64, String, String, String, DateTime<Utc>, DateTime<Utc>, String, i64, String);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT o.id, o.number, o.status, o.total_cents, c.name, c.provider, o.ship_address, o.expires_at, o.created_at,
                o.payment_ref, (SELECT COALESCE(SUM(qty),0)::bigint FROM social_order_items WHERE order_id=o.id),
                COALESCE((SELECT string_agg(code || ' x' || qty, ', ') FROM social_order_items WHERE order_id=o.id), '')
         FROM social_orders o JOIN social_customers c ON c.id = o.customer_id
         WHERE o.shop_id=$1 AND ($2::text IS NULL OR o.status=$2) AND ($3::uuid IS NULL OR o.session_id=$3)
           AND ($4::text IS NULL OR o.number ILIKE '%'||$4||'%' OR c.name ILIKE '%'||$4||'%' OR o.ship_phone ILIKE '%'||$4||'%')
         ORDER BY o.created_at DESC LIMIT $5",
    )
    .bind(shop_id)
    .bind(q.status.filter(|s| !s.is_empty()))
    .bind(q.session_id)
    .bind(q.q.filter(|s| !s.trim().is_empty()))
    .bind(q.limit.unwrap_or(200).clamp(1, 500))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows.into_iter().map(|(id, number, status, total, name, provider, addr, exp, at, pref, units, items)| json!({
        "id": id, "number": number, "status": status, "total_cents": total, "customer_name": name, "provider": provider,
        "has_address": !addr.is_empty(), "expires_at": exp, "created_at": at, "payment_ref": pref, "units": units, "items": items
    })).collect()))
}

async fn order_doc(st: &AppState, id: Uuid, include_private: bool) -> AppResult<Value> {
    let o: SocialOrder = sqlx::query_as("SELECT * FROM social_orders WHERE id=$1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let items: Vec<SocialItem> = sqlx::query_as(
        "SELECT id, product_id, code, name, qty, unit_price_cents FROM social_order_items WHERE order_id=$1 ORDER BY code",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id=$1").bind(o.shop_id).fetch_one(&st.db).await?;
    let customer: (String, String, String) = sqlx::query_as("SELECT name, provider, external_user_id FROM social_customers WHERE id=$1")
        .bind(o.customer_id)
        .fetch_one(&st.db)
        .await?;
    let mut v = json!({
        "order": o, "items": items,
        "customer": { "name": customer.0, "provider": customer.1 },
        "shop": {
            "name": shop.name, "slug": shop.slug, "logo_url": shop.logo_url, "legal_name": shop.legal_name, "tax_id": shop.tax_id,
            "branch": shop.branch, "address": shop.address, "phone": shop.phone, "currency": shop.currency,
            "receipt_footer": shop.receipt_footer, "payment_instructions": shop.payment_instructions
        }
    });
    if include_private {
        v["checkout_url"] = json!(format!("{}/c/{}", st.cfg.public_web_url.trim_end_matches('/'), o.token));
        v["customer"]["external_user_id"] = json!(customer.2);
        let comments: Vec<(String, String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT message, result, created_at FROM social_comments WHERE order_id=$1 ORDER BY created_at",
        )
        .bind(id)
        .fetch_all(&st.db)
        .await?;
        v["comments"] = json!(comments.into_iter().map(|(m, r, at)| json!({ "message": m, "result": r, "created_at": at })).collect::<Vec<_>>());
    }
    Ok(v)
}

async fn owned_order(st: &AppState, id: Uuid, user: &AuthUser) -> AppResult<Shop> {
    let shop_id: Uuid = sqlx::query_scalar("SELECT shop_id FROM social_orders WHERE id=$1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(st, shop_id, user).await
}

pub async fn get_order(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_order(&st, id, &user).await?;
    Ok(Json(order_doc(&st, id, true).await?))
}

#[derive(Deserialize)]
pub struct StatusReq {
    pub status: String,
    pub carrier_code: Option<String>,
    pub tracking_no: Option<String>,
    pub payment_ref: Option<String>,
    pub payment_method: Option<String>,
}

pub async fn set_status(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<StatusReq>) -> AppResult<Json<Value>> {
    owned_order(&st, id, &user).await?;
    let mut tx = st.db.begin().await?;
    let (current, number, method): (String, String, String) =
        sqlx::query_as("SELECT status, number, payment_method FROM social_orders WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let ok = matches!(
        (current.as_str(), r.status.as_str()),
        ("open", "confirmed") | ("open" | "confirmed", "paid") | ("paid", "shipped") | ("shipped", "completed")
            | ("open" | "confirmed" | "paid", "cancelled")
    ) || (method == "cod" && current == "confirmed" && r.status == "shipped");
    let carrier = r.carrier_code.as_deref().map(str::trim).filter(|c| !c.is_empty()).map(String::from);
    if let Some(c) = &carrier {
        let known: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM carriers WHERE code = $1)").bind(c).fetch_one(&mut *tx).await?;
        if !known {
            return Err(AppError::bad("unknown courier"));
        }
    }
    if !ok {
        return Err(AppError::bad(format!("cannot move a {current} order to {}", r.status)));
    }
    if r.status == "cancelled" {
        release(&mut tx, id, &number, "Cancelled").await?;
    }
    sqlx::query(
        "UPDATE social_orders SET status=$2, updated_at=now(),
            paid_at = CASE WHEN $2='paid' THEN now() ELSE paid_at END,
            confirmed_at = CASE WHEN $2 IN ('confirmed','paid') THEN COALESCE(confirmed_at, now()) ELSE confirmed_at END,
            tracking_no = COALESCE($3, tracking_no), payment_ref = COALESCE($4, payment_ref),
            payment_method = COALESCE($5, payment_method),
            carrier_code = COALESCE($6, carrier_code),
            shipped_at = CASE WHEN $2='shipped' THEN now() ELSE shipped_at END,
            cod_status = CASE WHEN $2='cancelled' AND cod_status='pending' THEN 'none'
                              WHEN $2='completed' AND cod_status='pending' THEN 'collected' ELSE cod_status END,
            cod_collected_at = CASE WHEN $2='completed' AND cod_status='pending' THEN now() ELSE cod_collected_at END
         WHERE id=$1",
    )
    .bind(id)
    .bind(&r.status)
    .bind(r.tracking_no)
    .bind(r.payment_ref)
    .bind(r.payment_method)
    .bind(carrier)
    .execute(&mut *tx)
    .await?;
    if r.status == "completed" {
        // Module hook (promo): loyalty points.
        crate::modules::promo::social_completed(&mut *tx, id).await?;
    }
    tx.commit().await?;
    let notify = notify_status(&st, id).await;
    let mut doc = order_doc(&st, id, true).await?;
    doc["notify"] = notify;
    Ok(Json(doc))
}

/// Message sent to the customer when the seller moves the order on. Language follows the shop currency
/// (LAK → Lao, THB → Thai, else English), like printed documents.
pub fn status_message(currency: &str, status: &str, number: &str, courier: &str, tracking: &str, track_url: &str, link: &str) -> Option<String> {
    let lang = match currency { "LAK" => "lo", "THB" => "th", _ => "en" };
    let ship = |label_courier: &str, label_no: &str| {
        let mut s = String::new();
        if !courier.is_empty() { s.push_str(&format!("\n{label_courier}: {courier}")); }
        if !tracking.is_empty() { s.push_str(&format!("\n{label_no}: {tracking}")); }
        if !track_url.is_empty() { s.push_str(&format!("\n{track_url}")); }
        s
    };
    let text = match (lang, status) {
        ("lo", "paid") => format!("✅ ໄດ້ຮັບເງິນແລ້ວ ຄຳສັ່ງຊື້ {number}. ກຳລັງກຽມສິນຄ້າໃຫ້ທ່ານ.\nຕິດຕາມ: {link}"),
        ("lo", "shipped") => format!("🚚 ຈັດສົ່ງຄຳສັ່ງຊື້ {number} ແລ້ວ!{}\nຕິດຕາມ: {link}", ship("ຂົນສົ່ງ", "ເລກພັດສະດຸ")),
        ("lo", "completed") => format!("🎉 ຄຳສັ່ງຊື້ {number} ສົ່ງຮອດແລ້ວ. ຂອບໃຈທີ່ອຸດໜູນ!"),
        ("lo", "cancelled") => format!("ຄຳສັ່ງຊື້ {number} ຖືກຍົກເລີກແລ້ວ. ມີຄຳຖາມ ຕອບກັບຂໍ້ຄວາມນີ້ໄດ້ເລີຍ."),
        ("th", "paid") => format!("✅ ได้รับชำระเงินออเดอร์ {number} แล้ว กำลังเตรียมจัดส่งค่ะ\nติดตาม: {link}"),
        ("th", "shipped") => format!("🚚 จัดส่งออเดอร์ {number} แล้วค่ะ!{}\nติดตาม: {link}", ship("ขนส่ง", "เลขพัสดุ")),
        ("th", "completed") => format!("🎉 ออเดอร์ {number} จัดส่งถึงแล้ว ขอบคุณที่อุดหนุนค่ะ"),
        ("th", "cancelled") => format!("ออเดอร์ {number} ถูกยกเลิกแล้ว สอบถามเพิ่มเติมตอบกลับข้อความนี้ได้เลยค่ะ"),
        (_, "paid") => format!("✅ Payment received for order {number}. We're packing it now.\nTrack: {link}"),
        (_, "shipped") => format!("🚚 Order {number} is on its way!{}\nTrack: {link}", ship("Courier", "Tracking no.")),
        (_, "completed") => format!("🎉 Order {number} was delivered. Thank you!"),
        (_, "cancelled") => format!("Order {number} was cancelled. Reply to this message if you have questions."),
        _ => return None,
    };
    Some(text)
}

/// Tells the customer about the order's new status in the chat it came from (Messenger / WhatsApp).
/// Never fails the status change: returns `{status: sent|failed|none|unsupported, error}`.
async fn notify_status(st: &AppState, order_id: Uuid) -> Value {
    let row: Option<(String, String, String, String, Option<String>, String, Option<Uuid>, String, String)> = sqlx::query_as(
        "SELECT o.status, o.number, o.token, o.tracking_no, o.carrier_code, o.currency, o.channel_id, c.provider, c.external_user_id
         FROM social_orders o JOIN social_customers c ON c.id = o.customer_id WHERE o.id = $1",
    )
    .bind(order_id)
    .fetch_optional(&st.db)
    .await
    .ok()
    .flatten();
    let Some((status, number, token, tracking, carrier, currency, channel_id, provider, user_id)) = row else {
        return json!({ "status": "none" });
    };
    let channel: Option<Channel> = match channel_id {
        Some(cid) => sqlx::query_as("SELECT * FROM social_channels WHERE id=$1").bind(cid).fetch_optional(&st.db).await.ok().flatten(),
        None => None,
    };
    let Some(c) = channel.filter(|c| c.active && c.auto_reply) else { return json!({ "status": "none" }) };
    let (courier, track_url) = match &carrier {
        Some(code) => {
            let r: Option<(String, String, String)> = sqlx::query_as("SELECT name, name_lo, tracking_url FROM carriers WHERE code=$1")
                .bind(code)
                .fetch_optional(&st.db)
                .await
                .ok()
                .flatten();
            match r {
                Some((name, name_lo, url)) => {
                    let name = if currency == "LAK" && !name_lo.is_empty() { name_lo } else { name };
                    let url = if url.contains("{tracking}") && !tracking.is_empty() { url.replace("{tracking}", &tracking) } else { String::new() };
                    (name, url)
                }
                None => (String::new(), String::new()),
            }
        }
        None => (String::new(), String::new()),
    };
    let link = format!("{}/c/{}", st.cfg.public_web_url.trim_end_matches('/'), token);
    let Some(text) = status_message(&currency, &status, &number, &courier, &tracking, &track_url, &link) else {
        return json!({ "status": "none" });
    };
    if user_id.is_empty() || c.access_token.is_empty() {
        return json!({ "status": "failed", "error": "no access token or customer id for this channel" });
    }
    let res = match (c.provider.as_str(), provider.as_str()) {
        // Outside the 24 h window Messenger only accepts tagged messages; POST_PURCHASE_UPDATE covers order updates.
        ("facebook", _) => graph_post(st, "me/messages", &c.access_token, json!({
            "recipient": { "id": user_id }, "messaging_type": "MESSAGE_TAG", "tag": "POST_PURCHASE_UPDATE", "message": { "text": text }
        })).await,
        // Outside the 24 h window WhatsApp needs an approved template; the error is returned to the seller.
        ("whatsapp", _) => graph_post(st, &format!("{}/messages", c.external_id), &c.access_token, json!({
            "messaging_product": "whatsapp", "to": user_id, "type": "text", "text": { "preview_url": true, "body": text }
        })).await,
        _ => return json!({ "status": "unsupported", "text": text }),
    };
    match res {
        Ok(_) => json!({ "status": "sent", "text": text }),
        Err(e) => json!({ "status": "failed", "error": e, "text": text }),
    }
}

#[derive(Deserialize)]
pub struct ItemReq {
    pub product_id: Uuid,
    /// New quantity; 0 removes the line.
    pub qty: i32,
}

/// Seller adjusts a line (stock difference is reserved/released).
pub async fn set_item(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<ItemReq>) -> AppResult<Json<Value>> {
    let shop = owned_order(&st, id, &user).await?;
    if !(0..=999).contains(&r.qty) {
        return Err(AppError::bad("qty must be 0–999"));
    }
    let mut tx = st.db.begin().await?;
    let (status, number): (String, String) = sqlx::query_as("SELECT status, number FROM social_orders WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    if !matches!(status.as_str(), "open" | "confirmed") {
        return Err(AppError::bad("only open or confirmed orders can be edited"));
    }
    let p: (String, Option<String>, i64, i32, Uuid) = sqlx::query_as(
        "SELECT name, social_code, price_cents, stock, shop_id FROM products WHERE id=$1 FOR UPDATE",
    )
    .bind(r.product_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if p.4 != shop.id {
        return Err(AppError::bad("product belongs to another shop"));
    }
    let current: i32 = sqlx::query_scalar("SELECT COALESCE(SUM(qty),0)::int FROM social_order_items WHERE order_id=$1 AND product_id=$2")
        .bind(id)
        .bind(r.product_id)
        .fetch_one(&mut *tx)
        .await?;
    let delta = r.qty - current;
    if delta > p.3 {
        return Err(AppError::bad(format!("only {} more in stock", p.3)));
    }
    if delta != 0 {
        move_stock(&mut tx, r.product_id, -delta, if delta > 0 { "reserve" } else { "release" }, id, &format!("Edit {number}")).await?;
    }
    if r.qty == 0 {
        sqlx::query("DELETE FROM social_order_items WHERE order_id=$1 AND product_id=$2").bind(id).bind(r.product_id).execute(&mut *tx).await?;
    } else {
        sqlx::query(
            "INSERT INTO social_order_items (order_id, product_id, code, name, qty, unit_price_cents) VALUES ($1,$2,$3,$4,$5,$6)
             ON CONFLICT (order_id, product_id) DO UPDATE SET qty = EXCLUDED.qty",
        )
        .bind(id)
        .bind(r.product_id)
        .bind(p.1.unwrap_or_default().to_uppercase())
        .bind(&p.0)
        .bind(r.qty)
        .bind(p.2)
        .execute(&mut *tx)
        .await?;
    }
    recalc(&mut tx, id, shop.social_shipping_cents).await?;
    tx.commit().await?;
    Ok(Json(order_doc(&st, id, true).await?))
}

/// Give every active product without a code the next free code (A01, A02, …).
pub async fn assign_codes(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let taken: Vec<String> = sqlx::query_scalar("SELECT upper(social_code) FROM products WHERE shop_id=$1 AND social_code IS NOT NULL")
        .bind(shop_id)
        .fetch_all(&st.db)
        .await?;
    let todo: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM products WHERE shop_id=$1 AND (social_code IS NULL OR social_code='') AND status <> 'archived' ORDER BY created_at",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    let mut n = 0usize;
    let mut next = 1;
    for pid in todo {
        let code = loop {
            let letter = (b'A' + ((next - 1) / 99) as u8) as char;
            let c = format!("{letter}{:02}", (next - 1) % 99 + 1);
            next += 1;
            if !taken.contains(&c) {
                break c;
            }
        };
        sqlx::query("UPDATE products SET social_code=$2 WHERE id=$1").bind(pid).bind(&code).execute(&st.db).await?;
        n += 1;
    }
    Ok(Json(json!({ "assigned": n })))
}

// ---------------------------------------------------------------------------
// Public checkout (customer, via secret link)
// ---------------------------------------------------------------------------

async fn by_token(st: &AppState, token: &str) -> AppResult<Uuid> {
    if token.len() < 20 {
        return Err(AppError::NotFound);
    }
    sqlx::query_scalar("SELECT id FROM social_orders WHERE token=$1")
        .bind(token)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn public_get(State(st): State<AppState>, Path(token): Path<String>) -> AppResult<Json<Value>> {
    let id = by_token(&st, &token).await?;
    let mut doc = order_doc(&st, id, false).await?;
    let shop_id: Uuid = sqlx::query_scalar("SELECT shop_id FROM social_orders WHERE id=$1").bind(id).fetch_one(&st.db).await?;
    doc["shipping_options"] = json!(logistics::options_for(&st.db, shop_id).await?);
    Ok(Json(doc))
}

/// A refused COD parcel came back: release the stock and cancel.
pub async fn cancel_returned(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> AppResult<()> {
    let number: String = sqlx::query_scalar("SELECT number FROM social_orders WHERE id=$1").bind(id).fetch_one(&mut **tx).await?;
    release(tx, id, &number, "Returned").await?;
    sqlx::query("UPDATE social_orders SET status='cancelled', updated_at=now() WHERE id=$1").bind(id).execute(&mut **tx).await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct ConfirmReq {
    pub name: String,
    pub phone: String,
    pub address: String,
    pub note: Option<String>,
    /// Courier chosen by the customer (required when the shop has couriers set up).
    pub carrier_code: Option<String>,
    /// branch | home
    pub delivery_type: Option<String>,
    /// Pay cash on delivery.
    #[serde(default)]
    pub cod: bool,
    /// Coupon code / loyalty points (module `promo`).
    #[serde(default)]
    pub promo: crate::modules::promo::PromoReq,
}

pub async fn public_confirm(
    State(st): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(token): Path<String>,
    Json(r): Json<ConfirmReq>,
) -> AppResult<Json<Value>> {
    let id = by_token(&st, &token).await?;
    if r.name.trim().is_empty() || r.phone.trim().len() < 6 || r.address.trim().len() < 5 {
        return Err(AppError::bad("please fill in your name, phone and full address"));
    }
    let (hold, shop_id, flat, subtotal): (i32, Uuid, i64, i64) = sqlx::query_as(
        "SELECT s.social_hold_hours, s.id, s.social_shipping_cents, o.subtotal_cents FROM social_orders o JOIN shops s ON s.id=o.shop_id WHERE o.id=$1",
    )
    .bind(id)
    .fetch_one(&st.db)
    .await?;
    let mut tx = st.db.begin().await?;
    let carrier = r.carrier_code.as_deref().map(str::trim).filter(|c| !c.is_empty());
    let delivery = r.delivery_type.as_deref().unwrap_or("branch");
    match carrier {
        // Validates the option (and COD) against the shop's current terms.
        Some(c) => {
            logistics::quote(&mut tx, shop_id, c, delivery, r.cod, subtotal).await?;
            if r.cod {
                // Module hook (cod_risk): no COD for customers an admin rated at/above COD_RISK_BLOCK_COD_AT.
                crate::modules::cod_risk::guard_cod(&mut tx, Some(r.phone.as_str())).await?;
            }
        }
        None if logistics::shop_has_shipping(&mut tx, shop_id).await? => {
            return Err(AppError::bad("choose a delivery option"));
        }
        None if r.cod => return Err(AppError::bad("cash on delivery is not available with this courier")),
        None => {}
    }
    let updated: Option<Uuid> = sqlx::query_scalar(
        "UPDATE social_orders SET status='confirmed', ship_name=$2, ship_phone=$3, ship_address=$4, customer_note=$5,
            confirmed_at=now(), expires_at = GREATEST(expires_at, now() + make_interval(hours => $6)), updated_at=now()
         WHERE id=$1 AND status IN ('open','confirmed') AND total_cents > 0 RETURNING id",
    )
    .bind(id)
    .bind(r.name.trim())
    .bind(r.phone.trim())
    .bind(r.address.trim())
    .bind(r.note.unwrap_or_default().chars().take(500).collect::<String>())
    .bind(hold)
    .fetch_optional(&mut *tx)
    .await?;
    if updated.is_none() {
        return Err(AppError::bad("this order can no longer be changed"));
    }
    sqlx::query(
        "UPDATE social_orders SET carrier_code=$2, delivery_type=$3,
            payment_method = CASE WHEN $4 THEN 'cod' WHEN payment_method = 'cod' THEN '' ELSE payment_method END,
            cod_status = CASE WHEN $4 THEN 'pending' ELSE 'none' END WHERE id=$1",
    )
    .bind(id)
    .bind(carrier)
    .bind(delivery)
    .bind(r.cod)
    .execute(&mut *tx)
    .await?;
    // Module hook (promo): re-price with the coupon/points the customer chose (a new confirm replaces the old choice).
    crate::modules::promo::sale_cancelled(&mut *tx, "social", id, None).await?;
    sqlx::query("UPDATE social_orders SET discount_cents = 0, platform_discount_cents = 0 WHERE id=$1").bind(id).execute(&mut *tx).await?;
    recalc(&mut tx, id, flat).await?;
    if !r.promo.is_empty() {
        let user = crate::auth::optional_user(&st, &headers);
        let ctx = crate::modules::promo::customer_social_ctx(&mut *tx, id, user, Some(r.phone.as_str())).await?;
        let a = crate::modules::promo::apply(&mut *tx, &ctx, &r.promo, true).await?;
        crate::modules::promo::commit(&mut *tx, &ctx, &a, "social", id, None).await?;
        sqlx::query("UPDATE social_orders SET discount_cents = $2, platform_discount_cents = $3 WHERE id=$1")
            .bind(id)
            .bind(a.discount_cents)
            .bind(a.platform_cents)
            .execute(&mut *tx)
            .await?;
        recalc(&mut tx, id, flat).await?;
    }
    tx.commit().await?;
    sqlx::query("UPDATE social_customers SET phone=$2, address=$3 WHERE id=(SELECT customer_id FROM social_orders WHERE id=$1)")
        .bind(id)
        .bind(r.phone.trim())
        .bind(r.address.trim())
        .execute(&st.db)
        .await?;
    Ok(Json(order_doc(&st, id, false).await?))
}

#[derive(Deserialize)]
pub struct PaymentReq {
    pub method: String,
    pub reference: String,
}

/// Customer tells the shop how/when they paid (e.g. transfer slip no.). The seller verifies and marks paid.
pub async fn public_payment(State(st): State<AppState>, Path(token): Path<String>, Json(r): Json<PaymentReq>) -> AppResult<Json<Value>> {
    let id = by_token(&st, &token).await?;
    if r.reference.trim().is_empty() {
        return Err(AppError::bad("enter the transfer reference or slip number"));
    }
    if r.method.trim() == "cod" {
        return Err(AppError::bad("choose cash on delivery with your delivery details"));
    }
    let updated: Option<Uuid> = sqlx::query_scalar(
        "UPDATE social_orders SET payment_method=$2, payment_ref=$3, updated_at=now() WHERE id=$1 AND status='confirmed' RETURNING id",
    )
    .bind(id)
    .bind(r.method.chars().take(30).collect::<String>())
    .bind(r.reference.trim().chars().take(120).collect::<String>())
    .fetch_optional(&st.db)
    .await?;
    if updated.is_none() {
        return Err(AppError::bad("confirm your shipping details first"));
    }
    Ok(Json(order_doc(&st, id, false).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_messages() {
        let m = status_message("LAK", "shipped", "S-0001", "Anousith", "AN123", "https://t/AN123", "https://x/c/tok").unwrap();
        assert!(m.contains("S-0001") && m.contains("AN123") && m.contains("https://x/c/tok"));
        assert!(status_message("THB", "paid", "S-1", "", "", "", "l").unwrap().contains("ออเดอร์"));
        assert!(status_message("USD", "open", "S-1", "", "", "", "l").is_none());
    }

    #[test]
    fn templates_and_money() {
        assert_eq!(money(123456, "THB"), "฿1,234.56");
        assert_eq!(money(90000, "THB"), "฿900");
        assert_eq!(render("hi {name}, {total} {link}", &[("name", "Bee".into()), ("total", "฿9".into()), ("link", "x".into())]), "hi Bee, ฿9 x");
    }
}
