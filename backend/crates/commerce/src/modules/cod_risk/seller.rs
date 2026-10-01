//! Seller side: refused parcels, filing reports, and checking customers before shipping.
//!
//! Sellers see the **level** and counts for any customer, but never other shops' reports,
//! names or notes.

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use super::{chain, cfg, identity, risk};
use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    routes::owned_shop,
    AppState,
};

pub const REASONS: [&str; 6] = ["refused", "unreachable", "fake_address", "no_show", "changed_mind", "other"];

/// A COD order of either kind (marketplace or comment/chat), with the fields a report needs.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CodOrder {
    pub kind: String,
    pub id: Uuid,
    pub number: String,
    pub customer: String,
    pub phone: String,
    pub carrier_code: Option<String>,
    pub tracking_no: String,
    pub cod_amount_cents: i64,
    pub currency: String,
    pub cod_status: String,
    pub order_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub buyer_id: Option<Uuid>,
    #[serde(skip)]
    pub shop_id: Option<Uuid>,
}

const COD_ORDERS: &str = "
    SELECT 'order' AS kind, o.id, upper(left(o.id::text, 8)) AS number,
           COALESCE(o.shipping_address->>'name', u.display_name) AS customer, COALESCE(o.shipping_address->>'phone', '') AS phone,
           o.carrier_code, o.tracking_no, o.cod_amount_cents, o.currency, o.cod_status, o.status AS order_status,
           o.created_at, o.updated_at, o.buyer_id,
           (SELECT oi.supplier_shop_id FROM order_items oi WHERE oi.order_id = o.id LIMIT 1) AS shop_id
    FROM orders o JOIN users u ON u.id = o.buyer_id WHERE o.cod_status <> 'none'
    UNION ALL
    SELECT 'social', s.id, s.number, s.ship_name, s.ship_phone, s.carrier_code, s.tracking_no, s.cod_amount_cents, s.currency,
           s.cod_status, s.status, s.created_at, s.updated_at, NULL::uuid, s.shop_id
    FROM social_orders s WHERE s.cod_status <> 'none'";

/// Level + counts for a set of customer keys (what any seller may see).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Signal {
    pub level: String,
    pub confirmed_reports: i64,
    pub shops: i64,
}

pub async fn signals(conn: &mut PgConnection, keys: &[String]) -> AppResult<HashMap<String, Signal>> {
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(&format!(
        "SELECT k.key, COALESCE((SELECT {} FROM cod_risk_customers c WHERE c.customer_key = k.key), 'none'),
                (SELECT count(*) FROM cod_risk_reports r WHERE r.customer_key = k.key AND r.status = 'confirmed'),
                (SELECT count(DISTINCT shop_id) FROM cod_risk_reports r WHERE r.customer_key = k.key AND r.status = 'confirmed')
         FROM unnest($1::text[]) AS k(key)",
        risk::EFFECTIVE_LEVEL_SQL
    ))
    .bind(keys)
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().map(|(k, level, confirmed_reports, shops)| (k, Signal { level, confirmed_reports, shops })).collect())
}

fn with_signal(o: &CodOrder, sig: &HashMap<String, Signal>) -> Value {
    let mut v = serde_json::to_value(o).unwrap_or_default();
    let s = identity::normalize(&o.phone).map(|p| identity::customer_key(&p)).and_then(|k| sig.get(&k).cloned());
    v["risk"] = json!(s);
    v
}

fn keys_of(rows: &[CodOrder]) -> Vec<String> {
    let mut k: Vec<String> = rows.iter().filter_map(|o| identity::normalize(&o.phone)).map(|p| identity::customer_key(&p)).collect();
    k.sort();
    k.dedup();
    k
}

/// Refused (returned) COD parcels of this shop, newest first, with their report if any.
pub async fn refused(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows: Vec<CodOrder> = sqlx::query_as(&format!(
        "SELECT * FROM ({COD_ORDERS}) x WHERE x.shop_id = $1 AND x.cod_status = 'returned' ORDER BY x.updated_at DESC LIMIT 300"
    ))
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    let reports: Vec<(String, Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT order_kind, order_id, id, status FROM cod_risk_reports WHERE shop_id = $1 AND order_id = ANY($2)",
    )
    .bind(shop_id)
    .bind(rows.iter().map(|r| r.id).collect::<Vec<_>>())
    .fetch_all(&st.db)
    .await?;
    let mut conn = st.db.acquire().await?;
    let sig = signals(&mut conn, &keys_of(&rows)).await?;
    let window = chrono::Duration::days(cfg().report_window_days);
    let out: Vec<Value> = rows
        .iter()
        .map(|o| {
            let mut v = with_signal(o, &sig);
            let rep = reports.iter().find(|r| r.0 == o.kind && r.1 == o.id);
            v["report"] = json!(rep.map(|r| json!({ "id": r.2, "status": r.3 })));
            v["reportable"] = json!(rep.is_none() && o.updated_at + window > Utc::now() && identity::normalize(&o.phone).is_some());
            v
        })
        .collect();
    Ok(Json(json!({ "rows": out, "window_days": cfg().report_window_days })))
}

/// COD orders not yet handed to the courier, with each customer's risk — check before shipping.
pub async fn unshipped(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows: Vec<CodOrder> = sqlx::query_as(&format!(
        "SELECT * FROM ({COD_ORDERS}) x WHERE x.shop_id = $1 AND x.cod_status = 'pending'
           AND ((x.kind = 'order' AND x.order_status IN ('pending','paid')) OR (x.kind = 'social' AND x.order_status IN ('open','confirmed','paid')))
         ORDER BY x.created_at DESC LIMIT 300"
    ))
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    let mut conn = st.db.acquire().await?;
    let sig = signals(&mut conn, &keys_of(&rows)).await?;
    Ok(Json(json!({ "rows": rows.iter().map(|o| with_signal(o, &sig)).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
pub struct CheckReq {
    pub phones: Vec<String>,
}

/// Look up phone numbers (e.g. a customer who calls to order). Level and counts only.
pub async fn check(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<CheckReq>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    if r.phones.is_empty() || r.phones.len() > 50 {
        return Err(AppError::bad("enter between 1 and 50 phone numbers"));
    }
    let parsed: Vec<(String, Option<String>)> = r.phones.iter().map(|p| (p.clone(), identity::normalize(p))).collect();
    let keys: Vec<String> = parsed.iter().filter_map(|(_, p)| p.as_deref().map(identity::customer_key)).collect();
    let mut conn = st.db.acquire().await?;
    let sig = signals(&mut conn, &keys).await?;
    let out: Vec<Value> = parsed
        .into_iter()
        .map(|(raw, p)| match p {
            None => json!({ "input": raw, "valid": false }),
            Some(p) => {
                let s = sig.get(&identity::customer_key(&p)).cloned().unwrap_or_default();
                json!({ "input": raw, "valid": true, "phone": p, "level": if s.level.is_empty() { "none".into() } else { s.level }, "confirmed_reports": s.confirmed_reports, "shops": s.shops })
            }
        })
        .collect();
    Ok(Json(json!({ "results": out })))
}

#[derive(Debug, Serialize, FromRow)]
pub struct ReportRow {
    pub id: Uuid,
    pub order_kind: String,
    pub order_id: Uuid,
    pub order_number: String,
    pub shop_id: Uuid,
    pub customer_key: String,
    pub customer_name: String,
    pub phone: String,
    pub reason: String,
    pub note: String,
    pub amount_cents: i64,
    pub currency: String,
    pub carrier_code: Option<String>,
    pub tracking_no: String,
    pub status: String,
    pub decision_note: String,
    pub decided_at: Option<DateTime<Utc>>,
    pub block_height: Option<i64>,
    pub created_at: DateTime<Utc>,
}

pub const REPORT_COLS: &str = "r.id, r.order_kind, r.order_id, r.order_number, r.shop_id, r.customer_key, r.customer_name, r.phone, r.reason,
    r.note, r.amount_cents, r.currency, r.carrier_code, r.tracking_no, r.status, r.decision_note, r.decided_at, r.block_height, r.created_at";

pub async fn my_reports(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Vec<ReportRow>>> {
    owned_shop(&st, shop_id, &user).await?;
    Ok(Json(
        sqlx::query_as(&format!("SELECT {REPORT_COLS} FROM cod_risk_reports r WHERE r.shop_id = $1 ORDER BY r.created_at DESC LIMIT 500"))
            .bind(shop_id)
            .fetch_all(&st.db)
            .await?,
    ))
}

#[derive(Deserialize)]
pub struct FileReq {
    pub kind: String,
    pub order_id: Uuid,
    pub reason: String,
    #[serde(default)]
    pub note: String,
}

/// File a report for one refused COD parcel of this shop.
pub async fn file_report(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<FileReq>) -> AppResult<Json<ReportRow>> {
    owned_shop(&st, shop_id, &user).await?;
    if !matches!(r.kind.as_str(), "order" | "social") {
        return Err(AppError::bad("kind must be order or social"));
    }
    if !REASONS.contains(&r.reason.as_str()) {
        return Err(AppError::bad("reason must be refused, unreachable, fake_address, no_show, changed_mind or other"));
    }
    let note: String = r.note.trim().chars().take(1000).collect();
    if r.reason == "other" && note.chars().count() < 5 {
        return Err(AppError::bad("describe what happened"));
    }
    let mut tx = st.db.begin().await?;
    let o: CodOrder = sqlx::query_as(&format!("SELECT * FROM ({COD_ORDERS}) x WHERE x.kind = $1 AND x.id = $2"))
        .bind(&r.kind)
        .bind(r.order_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
    if o.shop_id != Some(shop_id) {
        return Err(AppError::Forbidden);
    }
    if o.cod_status != "returned" {
        return Err(AppError::bad("only refused (returned) COD parcels can be reported"));
    }
    if o.updated_at + chrono::Duration::days(cfg().report_window_days) < Utc::now() {
        return Err(AppError::bad("this parcel came back too long ago to report"));
    }
    let phone = identity::normalize(&o.phone).ok_or_else(|| AppError::bad("this order has no valid customer phone number"))?;
    let key = identity::customer_key(&phone);
    sqlx::query(
        "INSERT INTO cod_risk_customers (customer_key, phone_hint) VALUES ($1, $2)
         ON CONFLICT (customer_key) DO UPDATE SET phone_hint = EXCLUDED.phone_hint",
    )
    .bind(&key)
    .bind(identity::hint(&phone))
    .execute(&mut *tx)
    .await?;
    let id = Uuid::new_v4();
    let block = chain::append(
        &mut tx,
        "report.filed",
        json!({
            "v": 1,
            "report_id": id,
            "order_kind": o.kind,
            "order_id": o.id,
            "shop_id": shop_id,
            "customer_key": key,
            "reason": r.reason,
            "amount_cents": o.cod_amount_cents,
            "currency": o.currency,
            "evidence_hash": chain::sha256_hex(format!("{}|{}|{}", note, o.carrier_code.clone().unwrap_or_default(), o.tracking_no)),
        }),
    )
    .await?;
    let row: ReportRow = sqlx::query_as(&format!(
        "INSERT INTO cod_risk_reports AS r (id, order_kind, order_id, order_number, shop_id, reporter_id, customer_key, buyer_id, customer_name,
                                       phone, reason, note, amount_cents, currency, carrier_code, tracking_no, block_height)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17) RETURNING {REPORT_COLS}"
    ))
    .bind(id)
    .bind(&o.kind)
    .bind(o.id)
    .bind(&o.number)
    .bind(shop_id)
    .bind(user.id)
    .bind(&key)
    .bind(o.buyer_id)
    .bind(o.customer.chars().take(200).collect::<String>())
    .bind(&phone)
    .bind(&r.reason)
    .bind(&note)
    .bind(o.cod_amount_cents)
    .bind(&o.currency)
    .bind(&o.carrier_code)
    .bind(&o.tracking_no)
    .bind(block.height)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict("this order has already been reported".into()),
        other => other,
    })?;
    tx.commit().await?;
    Ok(Json(row))
}

/// The reporting shop takes back a report the admin hasn't decided yet.
pub async fn withdraw(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let mut tx = st.db.begin().await?;
    let (shop_id, status, key): (Uuid, String, String) =
        sqlx::query_as("SELECT shop_id, status, customer_key FROM cod_risk_reports WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
    owned_shop(&st, shop_id, &user).await?;
    if status != "pending" {
        return Err(AppError::bad("only reports waiting for review can be withdrawn"));
    }
    let block = chain::append(&mut tx, "report.withdrawn", json!({ "v": 1, "report_id": id, "customer_key": key, "shop_id": shop_id })).await?;
    sqlx::query("UPDATE cod_risk_reports SET status = 'withdrawn', updated_at = now() WHERE id = $1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "id": id, "status": "withdrawn", "block_height": block.height })))
}
