//! Terminal ⇄ server: catalogue pull and sales push.

use std::collections::BTreeMap;

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    routes::pos::{move_stock, restock_voided, vat_for, Payment, METHODS},
    AppState,
};

use super::device::{has_perm, Device};

const PAGE: i64 = 500;
const MAX_BATCH: usize = 100;
const MAX_LINES: usize = 200;

fn shop_settings(d: &Device) -> Value {
    let s = &d.shop;
    json!({
        "id": s.id, "name": s.name, "slug": s.slug, "logo_url": s.logo_url, "currency": s.currency,
        "legal_name": s.legal_name, "tax_id": s.tax_id, "branch": s.branch, "address": s.address,
        "phone": s.phone, "vat_bps": s.vat_bps, "prices_include_vat": s.prices_include_vat,
        "receipt_prefix": s.receipt_prefix, "receipt_footer": s.receipt_footer,
    })
}

async fn device_info(st: &AppState, d: &Device) -> AppResult<Value> {
    let (last_seq, cashier): (i64, Option<String>) = sqlx::query_as(
        "SELECT COALESCE((SELECT MAX(device_seq) FROM pos_sales WHERE device_id = $1), 0)::bigint,
                (SELECT display_name FROM users WHERE id = $2)",
    )
    .bind(d.id)
    .bind(d.user.id)
    .fetch_one(&st.db)
    .await?;
    let can_void = has_perm(st, &d.shop, &d.user, "pos_void").await?;
    Ok(json!({
        "id": d.id, "code": d.code, "name": d.name, "last_seq": last_seq,
        "cashier": { "id": d.user.id, "name": cashier, "can_void": can_void },
    }))
}

/// The terminal, its cashier, and the shop settings.
pub async fn me(State(st): State<AppState>, d: Device) -> AppResult<Json<Value>> {
    Ok(Json(json!({ "device": device_info(&st, &d).await?, "shop": shop_settings(&d) })))
}

// ---------------------------------------------------------------------------
// Pull
// ---------------------------------------------------------------------------

/// Where a pull continues from.
///
/// * `c:<base>` — complete; everything committed before transaction `<base>` has been seen.
/// * `p:<base>:<since>:<txid>:<id>` — mid-way through a paged pull that started at `since`.
///
/// `base` is `pg_snapshot_xmin` of the first page's snapshot: every transaction still running then
/// has an id ≥ base, so rows it commits later are picked up by the next pull. Pages are ordered by
/// (txid, id); rows committed between pages with a smaller txid are also caught next time.
#[derive(Debug, PartialEq)]
struct Cursor {
    since: u64,
    base: Option<u64>,
    after: Option<(u64, Uuid)>,
}

fn parse_cursor(s: Option<&str>) -> AppResult<Cursor> {
    let bad = || AppError::bad("invalid sync cursor");
    let s = s.map(str::trim).unwrap_or("");
    if s.is_empty() || s == "0" {
        return Ok(Cursor { since: 0, base: None, after: None });
    }
    let parts: Vec<&str> = s.split(':').collect();
    let num = |p: &str| p.parse::<u64>().map_err(|_| bad());
    match parts.as_slice() {
        ["c", base] => Ok(Cursor { since: num(base)?, base: None, after: None }),
        ["p", base, since, txid, id] => Ok(Cursor {
            since: num(since)?,
            base: Some(num(base)?),
            after: Some((num(txid)?, id.parse().map_err(|_| bad())?)),
        }),
        _ => Err(bad()),
    }
}

#[derive(Deserialize)]
pub struct PullQ {
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

pub async fn pull(State(st): State<AppState>, d: Device, Query(q): Query<PullQ>) -> AppResult<Json<Value>> {
    let cur = parse_cursor(q.cursor.as_deref())?;
    let limit = q.limit.unwrap_or(PAGE).clamp(1, 2000);
    let mut tx = st.db.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(&mut *tx).await?;
    let xmin: String = sqlx::query_scalar("SELECT pg_snapshot_xmin(pg_current_snapshot())::text").fetch_one(&mut *tx).await?;
    let xmin: u64 = xmin.parse().map_err(|_| AppError::Internal("bad snapshot xmin".into()))?;
    let base = cur.base.unwrap_or(xmin).min(xmin);

    type Row = (Uuid, String, String, Option<String>, i64, i32, String, Option<Uuid>, Option<String>, DateTime<Utc>, String);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name, sku, barcode, price_cents, stock, status, shop_category_id,
                COALESCE(cover->>'thumb_url', cover->>'url', images->>0), updated_at, sync_txid::text
         FROM products
         WHERE shop_id = $1 AND sync_txid >= $2::text::xid8
           AND ($3::text IS NULL OR (sync_txid, id) > ($3::text::xid8, $4::uuid))
         ORDER BY sync_txid, id LIMIT $5",
    )
    .bind(d.shop.id)
    .bind(cur.since.to_string())
    .bind(cur.after.map(|a| a.0.to_string()))
    .bind(cur.after.map(|a| a.1))
    .bind(limit)
    .fetch_all(&mut *tx)
    .await?;
    let has_more = rows.len() as i64 == limit;

    let mut deleted: Vec<Uuid> = Vec::new();
    if !has_more && cur.since > 0 {
        deleted = sqlx::query_scalar("SELECT product_id FROM pos_sync_tombstones WHERE shop_id = $1 AND sync_txid >= $2::text::xid8")
            .bind(d.shop.id)
            .bind(cur.since.to_string())
            .fetch_all(&mut *tx)
            .await?;
    }
    let categories: Vec<(Uuid, Option<Uuid>, String, i32)> = sqlx::query_as(
        "SELECT id, parent_id, name, position FROM categories WHERE shop_id = $1 AND active ORDER BY position, name",
    )
    .bind(d.shop.id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let next = match (has_more, rows.last()) {
        (true, Some(last)) => format!("p:{base}:{}:{}:{}", cur.since, last.10, last.0),
        _ => format!("c:{base}"),
    };
    let products: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, sku, barcode, price, stock, status, cat, image, updated, _)| {
            json!({ "id": id, "name": name, "sku": sku, "barcode": barcode, "price_cents": price, "stock": stock,
                    "status": status, "shop_category_id": cat, "image": image, "updated_at": updated })
        })
        .collect();
    Ok(Json(json!({
        "cursor": next,
        "full": cur.since == 0,
        "has_more": has_more,
        "products": products,
        "deleted": deleted,
        "categories": categories.into_iter().map(|(id, parent, name, pos)| json!({ "id": id, "parent_id": parent, "name": name, "position": pos })).collect::<Vec<_>>(),
        "shop": shop_settings(&d),
        "device": device_info(&st, &d).await?,
        "server_time": Utc::now(),
    })))
}

// ---------------------------------------------------------------------------
// Push
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PushReq {
    #[serde(default)]
    pub sales: Vec<OfflineSale>,
    #[serde(default)]
    pub voids: Vec<OfflineVoid>,
}

#[derive(Deserialize, Clone)]
pub struct OfflineLine {
    pub product_id: Option<Uuid>,
    pub name: String,
    #[serde(default)]
    pub sku: String,
    pub qty: i32,
    pub unit_price_cents: i64,
    #[serde(default)]
    pub discount_cents: i64,
}

#[derive(Deserialize, Default, Clone)]
pub struct OfflineCustomer {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tax_id: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub phone: String,
}

#[derive(Deserialize, Clone)]
pub struct OfflineSale {
    /// Generated by the terminal; also the server id (idempotency key).
    pub id: Uuid,
    /// The terminal's receipt sequence number (1, 2, 3 …).
    pub seq: i64,
    pub number: String,
    pub created_at: DateTime<Utc>,
    pub items: Vec<OfflineLine>,
    #[serde(default)]
    pub discount_cents: i64,
    pub vat_bps: i32,
    pub prices_include_vat: bool,
    pub currency: String,
    pub subtotal_cents: i64,
    pub vat_cents: i64,
    pub total_cents: i64,
    pub paid_cents: i64,
    pub change_cents: i64,
    pub payments: Vec<Payment>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub customer: Option<OfflineCustomer>,
}

#[derive(Deserialize, Clone)]
pub struct OfflineVoid {
    pub sale_id: Uuid,
    pub reason: String,
    pub voided_at: Option<DateTime<Utc>>,
}

/// Check a sale the terminal recorded: the same arithmetic as the web till (`routes::pos`).
fn validate(s: &OfflineSale) -> Result<(), String> {
    if s.items.is_empty() || s.items.len() > MAX_LINES {
        return Err(format!("a sale needs 1 to {MAX_LINES} lines"));
    }
    if s.seq < 1 || s.number.trim().is_empty() || s.number.len() > 40 {
        return Err("invalid receipt number".into());
    }
    if !(0..=3000).contains(&s.vat_bps) {
        return Err("invalid VAT rate".into());
    }
    let mut subtotal = 0i64;
    for l in &s.items {
        if !(1..=10_000).contains(&l.qty) {
            return Err("qty must be between 1 and 10000".into());
        }
        if l.name.trim().is_empty() {
            return Err("every line needs a name".into());
        }
        if l.unit_price_cents < 0 {
            return Err("price cannot be negative".into());
        }
        let gross = l.unit_price_cents.checked_mul(l.qty as i64).ok_or("amount too large")?;
        if l.discount_cents < 0 || l.discount_cents > gross {
            return Err(format!("discount on '{}' is larger than the line", l.name.trim()));
        }
        subtotal += gross - l.discount_cents;
    }
    if s.discount_cents < 0 || s.discount_cents > subtotal {
        return Err("bill discount is larger than the subtotal".into());
    }
    let (vat, total) = vat_for(subtotal - s.discount_cents, s.vat_bps, s.prices_include_vat);
    if subtotal != s.subtotal_cents || vat != s.vat_cents || total != s.total_cents {
        return Err(format!("totals do not match (server: subtotal {subtotal}, VAT {vat}, total {total})"));
    }
    let (mut paid, mut cash) = (0i64, 0i64);
    for p in &s.payments {
        if !METHODS.contains(&p.method.as_str()) || p.amount_cents <= 0 {
            return Err("invalid payment".into());
        }
        paid += p.amount_cents;
        if p.method == "cash" {
            cash += p.amount_cents;
        }
    }
    if paid != s.paid_cents || paid < total || s.change_cents != paid - total || s.change_cents > cash {
        return Err("payments do not add up".into());
    }
    Ok(())
}

enum Outcome {
    Created { shortfall: Vec<Value> },
    Duplicate,
}

async fn store_sale(tx: &mut Transaction<'_, Postgres>, d: &Device, s: &OfflineSale) -> AppResult<Outcome> {
    // Already here? (a retry after a lost response)
    let existing: Option<Option<Uuid>> = sqlx::query_scalar("SELECT device_id FROM pos_sales WHERE id = $1").bind(s.id).fetch_optional(&mut **tx).await?;
    match existing {
        Some(Some(dev)) if dev == d.id => return Ok(Outcome::Duplicate),
        Some(_) => return Err(AppError::Conflict("sale id already used".into())),
        None => {}
    }
    let clash: Option<Uuid> = sqlx::query_scalar("SELECT id FROM pos_sales WHERE (device_id = $1 AND device_seq = $2) OR (shop_id = $3 AND number = $4)")
        .bind(d.id)
        .bind(s.seq)
        .bind(d.shop.id)
        .bind(s.number.trim())
        .fetch_optional(&mut **tx)
        .await?;
    if clash.is_some() {
        return Err(AppError::Conflict("receipt number already used".into()));
    }

    let mut ids: Vec<Uuid> = s.items.iter().filter_map(|l| l.product_id).collect();
    ids.sort();
    ids.dedup();
    let locked: Vec<(Uuid, i32)> = sqlx::query_as("SELECT id, stock FROM products WHERE id = ANY($1) AND shop_id = $2 ORDER BY id FOR UPDATE")
        .bind(&ids)
        .bind(d.shop.id)
        .fetch_all(&mut **tx)
        .await?;
    let stock: BTreeMap<Uuid, i32> = locked.into_iter().collect();

    let now = Utc::now();
    let created_at = s.created_at.min(now);
    let c = s.customer.clone().unwrap_or_default();
    let cl = |v: &str, n: usize| v.trim().chars().take(n).collect::<String>();
    sqlx::query(
        "INSERT INTO pos_sales (id, shop_id, number, cashier_id, subtotal_cents, discount_cents, vat_bps, prices_include_vat,
             vat_cents, total_cents, paid_cents, change_cents, payments, currency, note, customer_name, customer_tax_id,
             customer_branch, customer_address, customer_phone, created_at, origin, device_id, device_seq, synced_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,'device',$22,$23, now())",
    )
    .bind(s.id)
    .bind(d.shop.id)
    .bind(s.number.trim())
    .bind(d.user.id)
    .bind(s.subtotal_cents)
    .bind(s.discount_cents)
    .bind(s.vat_bps)
    .bind(s.prices_include_vat)
    .bind(s.vat_cents)
    .bind(s.total_cents)
    .bind(s.paid_cents)
    .bind(s.change_cents)
    .bind(serde_json::to_value(&s.payments).unwrap_or_default())
    .bind(cl(&s.currency, 3).to_uppercase())
    .bind(cl(&s.note, 500))
    .bind(cl(&c.name, 200))
    .bind(cl(&c.tax_id, 40))
    .bind(cl(&c.branch, 100))
    .bind(cl(&c.address, 500))
    .bind(cl(&c.phone, 40))
    .bind(created_at)
    .bind(d.id)
    .bind(s.seq)
    .execute(&mut **tx)
    .await?;

    let mut need: BTreeMap<Uuid, (i32, String)> = BTreeMap::new();
    for (i, l) in s.items.iter().enumerate() {
        // A product deleted since the terminal last synced: keep the line, drop the link.
        let pid = l.product_id.filter(|p| stock.contains_key(p));
        sqlx::query(
            "INSERT INTO pos_sale_items (sale_id, product_id, name, sku, qty, unit_price_cents, discount_cents, line_total_cents, position)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(s.id)
        .bind(pid)
        .bind(cl(&l.name, 120))
        .bind(cl(&l.sku, 80))
        .bind(l.qty)
        .bind(l.unit_price_cents)
        .bind(l.discount_cents)
        .bind(l.unit_price_cents * l.qty as i64 - l.discount_cents)
        .bind(i as i32)
        .execute(&mut **tx)
        .await?;
        if let Some(pid) = pid {
            let e = need.entry(pid).or_insert((0, l.name.trim().to_string()));
            e.0 += l.qty;
        }
    }
    let note = format!("POS {} ({})", s.number.trim(), d.code);
    let mut shortfall = Vec::new();
    for (pid, (qty, name)) in &need {
        let have = stock[pid].max(0);
        let take = (*qty).min(have);
        if take > 0 {
            move_stock(tx, *pid, -take, "sale", s.id, d.user.id, &note).await?;
        }
        if take < *qty {
            shortfall.push(json!({ "product_id": pid, "name": name, "qty": qty - take }));
        }
    }
    if !shortfall.is_empty() {
        sqlx::query("UPDATE pos_sales SET stock_shortfall = $2 WHERE id = $1").bind(s.id).bind(json!(shortfall)).execute(&mut **tx).await?;
    }
    Ok(Outcome::Created { shortfall })
}

async fn apply_void(tx: &mut Transaction<'_, Postgres>, d: &Device, v: &OfflineVoid) -> AppResult<bool> {
    let reason: String = v.reason.trim().chars().take(300).collect();
    if reason.is_empty() {
        return Err(AppError::bad("give a reason for voiding"));
    }
    let at = v.voided_at.unwrap_or_else(Utc::now).min(Utc::now());
    let number: Option<String> = sqlx::query_scalar(
        "UPDATE pos_sales SET status='voided', void_reason=$3, voided_at=$4, voided_by=$5
         WHERE id=$1 AND shop_id=$2 AND status='completed' RETURNING number",
    )
    .bind(v.sale_id)
    .bind(d.shop.id)
    .bind(&reason)
    .bind(at)
    .bind(d.user.id)
    .fetch_optional(&mut **tx)
    .await?;
    match number {
        Some(n) => {
            restock_voided(tx, v.sale_id, d.user.id, &n).await?;
            Ok(true)
        }
        None => {
            let status: Option<String> = sqlx::query_scalar("SELECT status FROM pos_sales WHERE id=$1 AND shop_id=$2")
                .bind(v.sale_id)
                .bind(d.shop.id)
                .fetch_optional(&mut **tx)
                .await?;
            match status.as_deref() {
                Some("voided") => Ok(false),
                _ => Err(AppError::NotFound),
            }
        }
    }
}

fn message(e: &AppError, lao: bool) -> String {
    let m = e.to_string();
    if lao {
        crate::i18n::translate_lo(&m).unwrap_or(m)
    } else {
        m
    }
}

/// Upload sales and voids recorded on the terminal. Each item is applied in its own transaction and
/// gets its own result, so one bad sale never blocks the rest of the queue.
pub async fn push(State(st): State<AppState>, headers: HeaderMap, d: Device, Json(req): Json<PushReq>) -> AppResult<Json<Value>> {
    if req.sales.len() > MAX_BATCH || req.voids.len() > MAX_BATCH {
        return Err(AppError::bad("send at most 100 sales and 100 voids at a time"));
    }
    let lao = crate::i18n::wants_lao(&headers);
    let mut sales = Vec::with_capacity(req.sales.len());
    for s in &req.sales {
        if let Err(e) = validate(s) {
            let msg = if lao { crate::i18n::translate_lo(&e).unwrap_or(e) } else { e };
            sales.push(json!({ "id": s.id, "status": "rejected", "error": msg }));
            continue;
        }
        let mut tx = st.db.begin().await?;
        match store_sale(&mut tx, &d, s).await {
            Ok(Outcome::Created { shortfall }) => {
                tx.commit().await?;
                sales.push(json!({ "id": s.id, "status": "ok", "shortfall": shortfall }));
            }
            Ok(Outcome::Duplicate) => sales.push(json!({ "id": s.id, "status": "duplicate" })),
            Err(e @ (AppError::Internal(_) | AppError::Upstream(_))) => return Err(e),
            Err(e) => sales.push(json!({ "id": s.id, "status": "rejected", "error": message(&e, lao) })),
        }
    }

    let can_void = !req.voids.is_empty() && has_perm(&st, &d.shop, &d.user, "pos_void").await?;
    let mut voids = Vec::with_capacity(req.voids.len());
    for v in &req.voids {
        if !can_void {
            voids.push(json!({ "sale_id": v.sale_id, "status": "rejected", "error": message(&AppError::Forbidden, lao) }));
            continue;
        }
        let mut tx = st.db.begin().await?;
        match apply_void(&mut tx, &d, v).await {
            Ok(changed) => {
                tx.commit().await?;
                voids.push(json!({ "sale_id": v.sale_id, "status": if changed { "ok" } else { "duplicate" } }));
            }
            Err(e @ AppError::Internal(_)) => return Err(e),
            Err(e) => voids.push(json!({ "sale_id": v.sale_id, "status": "rejected", "error": message(&e, lao) })),
        }
    }
    if !req.sales.is_empty() || !req.voids.is_empty() {
        sqlx::query("UPDATE pos_devices SET last_push_at = now() WHERE id = $1").bind(d.id).execute(&st.db).await?;
    }
    Ok(Json(json!({ "sales": sales, "voids": voids, "server_time": Utc::now() })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sale() -> OfflineSale {
        OfflineSale {
            id: Uuid::new_v4(),
            seq: 1,
            number: "RT01-000001".into(),
            created_at: Utc::now(),
            items: vec![
                OfflineLine { product_id: None, name: "Coffee".into(), sku: String::new(), qty: 2, unit_price_cents: 2500, discount_cents: 0 },
                OfflineLine { product_id: None, name: "Cake".into(), sku: String::new(), qty: 1, unit_price_cents: 4000, discount_cents: 500 },
            ],
            discount_cents: 500,
            vat_bps: 700,
            prices_include_vat: true,
            currency: "LAK".into(),
            subtotal_cents: 8500,
            vat_cents: 523,
            total_cents: 8000,
            paid_cents: 10000,
            change_cents: 2000,
            payments: vec![Payment { method: "cash".into(), amount_cents: 10000, reference: String::new() }],
            note: String::new(),
            customer: None,
        }
    }

    #[test]
    fn validates_like_the_web_till() {
        assert_eq!(validate(&sale()), Ok(()));
        let mut s = sale();
        s.total_cents += 1;
        assert!(validate(&s).unwrap_err().starts_with("totals do not match"));
        let mut s = sale();
        s.payments = vec![Payment { method: "card".into(), amount_cents: 10000, reference: String::new() }];
        assert_eq!(validate(&s).unwrap_err(), "payments do not add up"); // change only from cash
        let mut s = sale();
        s.items[1].discount_cents = 9000;
        assert!(validate(&s).is_err());
    }

    #[test]
    fn cursors() {
        assert_eq!(parse_cursor(None).unwrap(), Cursor { since: 0, base: None, after: None });
        assert_eq!(parse_cursor(Some("c:42")).unwrap(), Cursor { since: 42, base: None, after: None });
        let id = Uuid::new_v4();
        assert_eq!(parse_cursor(Some(&format!("p:40:0:41:{id}"))).unwrap(), Cursor { since: 0, base: Some(40), after: Some((41, id)) });
        assert!(parse_cursor(Some("x:1")).is_err());
        assert!(parse_cursor(Some("c:-1")).is_err());
    }
}
