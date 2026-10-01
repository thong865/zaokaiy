//! Admin side: review reports, decide customers' risk levels, inspect and anchor the ledger.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use super::{
    chain, cfg, identity,
    risk::{self, Level, EFFECTIVE_LEVEL_SQL as EFF},
    seller::REPORT_COLS,
};
use crate::{
    auth::AdminUser,
    error::{AppError, AppResult},
    AppState,
};

const LEVEL_RANK: &str = "array_position(ARRAY['none','low','medium','high','blocked'], {EFF})";

pub async fn overview(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Value>> {
    let reports: Vec<(String, i64)> = sqlx::query_as("SELECT status, count(*) FROM cod_risk_reports GROUP BY status").fetch_all(&st.db).await?;
    let levels: Vec<(String, i64)> = sqlx::query_as(&format!("SELECT {EFF}, count(*) FROM cod_risk_customers GROUP BY 1")).fetch_all(&st.db).await?;
    let (height, unanchored): (Option<i64>, i64) =
        sqlx::query_as("SELECT max(height), count(*) FILTER (WHERE anchor_id IS NULL) FROM cod_risk_blocks").fetch_one(&st.db).await?;
    Ok(Json(json!({
        "reports": reports.into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        "levels": levels.into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        "chain": { "height": height.unwrap_or(0), "unanchored": unanchored },
        "config": super::public_status(),
    })))
}

#[derive(Debug, Serialize, FromRow)]
pub struct AdminReport {
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
    pub shop_name: String,
    pub level: String,
    pub customer_confirmed: i64,
    pub customer_reports: i64,
}

#[derive(Deserialize)]
pub struct ListQ {
    pub status: Option<String>,
    pub level: Option<String>,
    pub q: Option<String>,
}

/// `q` matches a phone number exactly (any format) or a name / shop / order number.
fn search_terms(q: &Option<String>) -> (Option<String>, Option<String>) {
    let q = q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let key = q.and_then(identity::normalize).map(|p| identity::customer_key(&p));
    (key, q.map(|s| s.chars().take(80).collect()))
}

pub async fn reports(State(st): State<AppState>, _a: AdminUser, Query(q): Query<ListQ>) -> AppResult<Json<Value>> {
    let status = q.status.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "pending".into());
    let (key, text) = search_terms(&q.q);
    let items: Vec<AdminReport> = sqlx::query_as(&format!(
        "SELECT {REPORT_COLS}, s.name AS shop_name, {EFF} AS level,
                (SELECT count(*) FROM cod_risk_reports x WHERE x.customer_key = r.customer_key AND x.status = 'confirmed') AS customer_confirmed,
                (SELECT count(*) FROM cod_risk_reports x WHERE x.customer_key = r.customer_key) AS customer_reports
         FROM cod_risk_reports r JOIN shops s ON s.id = r.shop_id JOIN cod_risk_customers c ON c.customer_key = r.customer_key
         WHERE ($1 = 'all' OR r.status = $1)
           AND ($2::text IS NULL AND $3::text IS NULL OR r.customer_key = $2
                OR r.customer_name ILIKE '%'||$3||'%' OR s.name ILIKE '%'||$3||'%' OR r.order_number ILIKE '%'||$3||'%')
         ORDER BY r.created_at {} LIMIT 300",
        if status == "pending" { "ASC" } else { "DESC" }
    ))
    .bind(&status)
    .bind(key)
    .bind(text)
    .fetch_all(&st.db)
    .await?;
    let counts: Vec<(String, i64)> = sqlx::query_as("SELECT status, count(*) FROM cod_risk_reports GROUP BY status").fetch_all(&st.db).await?;
    Ok(Json(json!({ "items": items, "counts": counts.into_iter().collect::<std::collections::BTreeMap<_, _>>() })))
}

#[derive(Deserialize)]
pub struct DecideReq {
    /// confirm | dismiss
    pub action: String,
    #[serde(default)]
    pub note: String,
    /// Optionally set the customer's level in the same step.
    pub level: Option<String>,
    pub expires_days: Option<i64>,
}

pub async fn decide(State(st): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<Uuid>, Json(r): Json<DecideReq>) -> AppResult<Json<Value>> {
    let next = match r.action.as_str() {
        "confirm" => "confirmed",
        "dismiss" => "dismissed",
        _ => return Err(AppError::bad("action must be confirm or dismiss")),
    };
    let note: String = r.note.trim().chars().take(1000).collect();
    let mut tx = st.db.begin().await?;
    let (status, key, shop_id): (String, String, Uuid) =
        sqlx::query_as("SELECT status, customer_key, shop_id FROM cod_risk_reports WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
    if status == next || status == "withdrawn" {
        return Err(AppError::bad(format!("cannot {} a report that is {status}", r.action)));
    }
    // Dismissing, or changing an earlier decision (e.g. after the customer appeals), needs a reason.
    if (next == "dismissed" || status != "pending") && note.is_empty() {
        return Err(AppError::bad("add a note explaining the decision"));
    }
    chain::append(
        &mut tx,
        &format!("report.{next}"),
        json!({ "v": 1, "report_id": id, "customer_key": key, "shop_id": shop_id, "previous": status, "by": admin.id, "note_hash": chain::sha256_hex(&note) }),
    )
    .await?;
    sqlx::query("UPDATE cod_risk_reports SET status = $2, decision_note = $3, decided_by = $4, decided_at = now(), updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(next)
        .bind(&note)
        .bind(admin.id)
        .execute(&mut *tx)
        .await?;
    if let Some(level) = r.level.as_deref().filter(|l| !l.is_empty()) {
        set_level_tx(&mut tx, &key, level, &note, r.expires_days, admin.id).await?;
    }
    tx.commit().await?;
    customer_doc(&st, &key).await.map(Json)
}

#[derive(Deserialize)]
pub struct LevelReq {
    pub level: String,
    #[serde(default)]
    pub note: String,
    pub expires_days: Option<i64>,
}

pub async fn set_level(State(st): State<AppState>, AdminUser(admin): AdminUser, Path(key): Path<String>, Json(r): Json<LevelReq>) -> AppResult<Json<Value>> {
    let mut tx = st.db.begin().await?;
    set_level_tx(&mut tx, &key, &r.level, r.note.trim(), r.expires_days, admin.id).await?;
    tx.commit().await?;
    customer_doc(&st, &key).await.map(Json)
}

async fn set_level_tx(conn: &mut PgConnection, key: &str, level: &str, note: &str, expires_days: Option<i64>, by: Uuid) -> AppResult<()> {
    let level = Level::parse(level).ok_or_else(|| AppError::bad("level must be none, low, medium, high or blocked"))?;
    if expires_days.is_some_and(|d| !(1..=3650).contains(&d)) {
        return Err(AppError::bad("expiry must be between 1 and 3650 days"));
    }
    let note: String = note.chars().take(1000).collect();
    if level != Level::None && note.is_empty() {
        return Err(AppError::bad("add a note explaining the risk level"));
    }
    let prev: String = sqlx::query_scalar("SELECT level FROM cod_risk_customers WHERE customer_key = $1 FOR UPDATE")
        .bind(key)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(AppError::NotFound)?;
    let expires_at = expires_days.filter(|_| level != Level::None).map(|d| Utc::now() + chrono::Duration::days(d));
    chain::append(
        &mut *conn,
        "level.set",
        json!({
            "v": 1,
            "customer_key": key,
            "level": level.as_str(),
            "previous": prev,
            "expires_at": expires_at.map(|t| t.to_rfc3339()),
            "by": by,
            "note_hash": chain::sha256_hex(&note),
        }),
    )
    .await?;
    sqlx::query(
        "UPDATE cod_risk_customers SET level = $2, level_note = $3, level_set_by = $4, level_set_at = now(),
                level_expires_at = $5, updated_at = now() WHERE customer_key = $1",
    )
    .bind(key)
    .bind(level.as_str())
    .bind(&note)
    .bind(by)
    .bind(expires_at)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

#[derive(Debug, Serialize, FromRow)]
pub struct CustomerRow {
    pub customer_key: String,
    pub phone_hint: String,
    pub phone: Option<String>,
    pub name: Option<String>,
    pub level: String,
    pub effective_level: String,
    pub level_note: String,
    pub level_set_at: Option<DateTime<Utc>>,
    pub level_expires_at: Option<DateTime<Utc>>,
    pub confirmed: i64,
    pub pending: i64,
    pub shops: i64,
    pub updated_at: DateTime<Utc>,
}

fn customer_select() -> String {
    format!(
        "SELECT c.customer_key, c.phone_hint,
                (SELECT r.phone FROM cod_risk_reports r WHERE r.customer_key = c.customer_key ORDER BY r.created_at DESC LIMIT 1) AS phone,
                (SELECT r.customer_name FROM cod_risk_reports r WHERE r.customer_key = c.customer_key ORDER BY r.created_at DESC LIMIT 1) AS name,
                c.level, {EFF} AS effective_level, c.level_note, c.level_set_at, c.level_expires_at,
                (SELECT count(*) FROM cod_risk_reports r WHERE r.customer_key = c.customer_key AND r.status = 'confirmed') AS confirmed,
                (SELECT count(*) FROM cod_risk_reports r WHERE r.customer_key = c.customer_key AND r.status = 'pending') AS pending,
                (SELECT count(DISTINCT r.shop_id) FROM cod_risk_reports r WHERE r.customer_key = c.customer_key AND r.status = 'confirmed') AS shops,
                c.updated_at
         FROM cod_risk_customers c"
    )
}

pub async fn customers(State(st): State<AppState>, _a: AdminUser, Query(q): Query<ListQ>) -> AppResult<Json<Vec<CustomerRow>>> {
    let level = q.level.clone().filter(|s| !s.is_empty() && s != "all");
    let (key, text) = search_terms(&q.q);
    let rank = LEVEL_RANK.replace("{EFF}", EFF);
    Ok(Json(
        sqlx::query_as(&format!(
            "{} WHERE ($1::text IS NULL OR ($1 = 'flagged' AND {EFF} <> 'none') OR {EFF} = $1)
               AND ($2::text IS NULL AND $3::text IS NULL OR c.customer_key = $2
                    OR EXISTS (SELECT 1 FROM cod_risk_reports r WHERE r.customer_key = c.customer_key AND r.customer_name ILIKE '%'||$3||'%'))
             ORDER BY {rank} DESC, c.updated_at DESC LIMIT 300",
            customer_select()
        ))
        .bind(level)
        .bind(key)
        .bind(text)
        .fetch_all(&st.db)
        .await?,
    ))
}

pub async fn customer(State(st): State<AppState>, _a: AdminUser, Path(key): Path<String>) -> AppResult<Json<Value>> {
    customer_doc(&st, &key).await.map(Json)
}

async fn customer_doc(st: &AppState, key: &str) -> AppResult<Value> {
    let c: CustomerRow = sqlx::query_as(&format!("{} WHERE c.customer_key = $1", customer_select()))
        .bind(key)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let mut conn = st.db.acquire().await?;
    let stats = risk::stats(&mut conn, key).await?;
    let reports: Vec<Value> = sqlx::query_as::<_, (sqlx::types::Json<Value>,)>(&format!(
        "SELECT to_jsonb(x) FROM (SELECT {REPORT_COLS}, s.name AS shop_name FROM cod_risk_reports r JOIN shops s ON s.id = r.shop_id
         WHERE r.customer_key = $1 ORDER BY r.created_at DESC) x"
    ))
    .bind(key)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|r| r.0 .0)
    .collect();
    let ledger: Vec<Value> = sqlx::query_as::<_, (i64, String, String, String, i64, Option<String>)>(
        "SELECT b.height, b.hash, b.event, b.payload, b.ts_micros, a.tx_ref FROM cod_risk_blocks b
         LEFT JOIN cod_risk_anchors a ON a.id = b.anchor_id
         WHERE b.payload::jsonb->>'customer_key' = $1 ORDER BY b.height",
    )
    .bind(key)
    .fetch_all(&mut *conn)
    .await?
    .into_iter()
    .map(|(height, hash, event, payload, ts, tx)| {
        json!({ "height": height, "hash": hash, "event": event, "payload": serde_json::from_str::<Value>(&payload).unwrap_or_default(),
                "at": DateTime::<Utc>::from_timestamp_micros(ts), "anchor_tx": tx })
    })
    .collect();
    Ok(json!({ "customer": c, "stats": stats, "suggested_level": risk::suggest(&stats), "reports": reports, "ledger": ledger }))
}

pub async fn chain_status(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Value>> {
    let (height, tip, verified) = chain::verify_all(&st.db).await?;
    let unanchored: i64 = sqlx::query_scalar("SELECT count(*) FROM cod_risk_blocks WHERE anchor_id IS NULL").fetch_one(&st.db).await?;
    let anchors: Vec<chain::Anchor> = sqlx::query_as("SELECT * FROM cod_risk_anchors ORDER BY id DESC LIMIT 20").fetch_all(&st.db).await?;
    let recent: Vec<Value> = sqlx::query_as::<_, (i64, String, String, i64, Option<i64>)>(
        "SELECT height, hash, event, ts_micros, anchor_id FROM cod_risk_blocks ORDER BY height DESC LIMIT 20",
    )
    .fetch_all(&st.db)
    .await?
    .into_iter()
    .map(|(h, hash, event, ts, a)| json!({ "height": h, "hash": hash, "event": event, "at": DateTime::<Utc>::from_timestamp_micros(ts), "anchor_id": a }))
    .collect();
    let c = cfg();
    Ok(Json(json!({
        "height": height,
        "tip_hash": tip,
        "valid": verified.is_ok(),
        "broken": verified.err(),
        "unanchored": unanchored,
        "anchors": anchors,
        "recent": recent,
        "driver": c.anchor,
        "anchoring": c.anchoring(),
        "ledger": c.ledger_name,
    })))
}

pub async fn anchor_now(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Value>> {
    let a = chain::anchor_pending(&st).await?;
    Ok(Json(json!({ "anchor": a })))
}

/// Everything needed to verify one block independently: its content, the anchored batch it
/// belongs to, and the Merkle path from the block to the root published on-chain.
pub async fn proof(State(st): State<AppState>, _a: AdminUser, Path(height): Path<i64>) -> AppResult<Json<Value>> {
    let b: chain::Block = sqlx::query_as("SELECT height, prev_hash, hash, event, payload, ts_micros, anchor_id FROM cod_risk_blocks WHERE height = $1")
        .bind(height)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let recomputed = chain::block_hash(b.height, &b.prev_hash, b.ts_micros, &b.event, &b.payload);
    let mut out = json!({ "block": b, "hash_ok": recomputed == b.hash, "anchor": null, "proof": null, "root_ok": null });
    if let Some(aid) = b.anchor_id {
        let a: chain::Anchor = sqlx::query_as("SELECT * FROM cod_risk_anchors WHERE id = $1").bind(aid).fetch_one(&st.db).await?;
        let hashes: Vec<String> = sqlx::query_scalar("SELECT hash FROM cod_risk_blocks WHERE height BETWEEN $1 AND $2 ORDER BY height")
            .bind(a.from_height)
            .bind(a.to_height)
            .fetch_all(&st.db)
            .await?;
        let idx = (b.height - a.from_height) as usize;
        let p = chain::merkle_proof(&hashes, idx);
        out["root_ok"] = json!(chain::merkle_root(&hashes) == a.merkle_root && chain::verify_proof(&b.hash, &p, &a.merkle_root));
        out["proof"] = json!(p);
        out["anchor"] = json!(a);
    }
    Ok(Json(out))
}
