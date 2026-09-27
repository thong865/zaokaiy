//! Point of sale: counter selling with receipts and full tax invoices.
//!
//! * Money is integer minor units; VAT is computed per shop settings (`vat_bps`, prices incl./excl. VAT)
//!   and frozen onto each sale so later setting changes never alter issued documents.
//! * Receipt / invoice numbers are gap-free per shop (`shop_counters`, row-locked in the sale tx).
//! * Stock is decremented in the same transaction; voiding a sale restocks and keeps the number.
//! * Counter sales ignore marketplace review — it's the shop's own till — but archived products
//!   can't be sold.

use std::collections::BTreeMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::Shop,
    routes::owned_shop,
    AppState,
};

pub const METHODS: [&str; 5] = ["cash", "card", "transfer", "qr", "other"];
const MAX_LINES: usize = 200;

#[derive(Debug, Serialize, FromRow)]
pub struct PosSale {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub number: String,
    pub cashier_id: Option<Uuid>,
    pub status: String,
    pub subtotal_cents: i64,
    pub discount_cents: i64,
    pub vat_bps: i32,
    pub prices_include_vat: bool,
    pub vat_cents: i64,
    pub total_cents: i64,
    pub paid_cents: i64,
    pub change_cents: i64,
    pub payments: Value,
    pub currency: String,
    pub note: String,
    pub invoice_number: Option<String>,
    pub invoice_issued_at: Option<DateTime<Utc>>,
    pub customer_name: String,
    pub customer_tax_id: String,
    pub customer_branch: String,
    pub customer_address: String,
    pub customer_phone: String,
    pub void_reason: String,
    pub voided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct PosItem {
    pub id: Uuid,
    pub product_id: Option<Uuid>,
    pub name: String,
    pub sku: String,
    pub qty: i32,
    pub unit_price_cents: i64,
    pub discount_cents: i64,
    pub line_total_cents: i64,
}

/// Round-half-up integer division.
fn div_round(a: i64, b: i64) -> i64 {
    (a * 2 + b) / (b * 2)
}

/// Returns (vat, total) for a net amount after discounts.
pub fn vat_for(net: i64, bps: i32, inclusive: bool) -> (i64, i64) {
    let bps = bps as i64;
    if bps == 0 {
        return (0, net);
    }
    if inclusive {
        (div_round(net * bps, 10_000 + bps), net)
    } else {
        let vat = div_round(net * bps, 10_000);
        (vat, net + vat)
    }
}

async fn next_number(tx: &mut Transaction<'_, Postgres>, shop_id: Uuid, kind: &str, prefix: &str) -> AppResult<String> {
    let n: i64 = sqlx::query_scalar(
        "INSERT INTO shop_counters (shop_id, kind, next) VALUES ($1, $2, 2)
         ON CONFLICT (shop_id, kind) DO UPDATE SET next = shop_counters.next + 1
         RETURNING next - 1",
    )
    .bind(shop_id)
    .bind(kind)
    .fetch_one(&mut **tx)
    .await?;
    Ok(format!("{prefix}{n:06}"))
}

// ---------------------------------------------------------------------------
// Product lookup
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SearchQ {
    pub q: Option<String>,
    pub shop_category_id: Option<Uuid>,
    pub limit: Option<i64>,
}

/// Fast product lookup for the till. An exact barcode/SKU match comes first so a scanner
/// (which "types" the code + Enter) adds the right item instantly.
pub async fn search(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<SearchQ>,
) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, shop_id, &user).await?;
    let term = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    type Row = (Uuid, String, String, Option<String>, i64, i32, Value, String, Option<Uuid>, bool);
    let rows: Vec<Row> = sqlx::query_as(
        "WITH RECURSIVE sub AS (
            SELECT id FROM categories WHERE id = $3
            UNION ALL SELECT c.id FROM categories c JOIN sub ON c.parent_id = sub.id
         )
         SELECT id, name, sku, barcode, price_cents, stock, images, status, shop_category_id,
                COALESCE(lower(sku) = lower($2) OR barcode = $2, false) AS exact
         FROM products
         WHERE shop_id = $1 AND status <> 'archived'
           AND ($2::text IS NULL OR name ILIKE '%'||$2||'%' OR sku ILIKE '%'||$2||'%' OR barcode = $2)
           AND ($3::uuid IS NULL OR shop_category_id IN (SELECT id FROM sub))
         ORDER BY exact DESC NULLS LAST, name
         LIMIT $4",
    )
    .bind(shop_id)
    .bind(term)
    .bind(q.shop_category_id)
    .bind(q.limit.unwrap_or(60).clamp(1, 200))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, name, sku, barcode, price, stock, images, status, cat, exact)| {
                json!({ "id": id, "name": name, "sku": sku, "barcode": barcode, "price_cents": price,
                        "stock": stock, "image": images.get(0), "status": status,
                        "shop_category_id": cat, "exact": exact })
            })
            .collect(),
    ))
}

// ---------------------------------------------------------------------------
// Checkout
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LineReq {
    pub product_id: Option<Uuid>,
    /// Required for custom (non-catalogue) items.
    pub name: Option<String>,
    pub qty: i32,
    /// Price override; defaults to the product price. Required for custom items.
    pub unit_price_cents: Option<i64>,
    pub discount_cents: Option<i64>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Payment {
    pub method: String,
    pub amount_cents: i64,
    #[serde(default)]
    pub reference: String,
}

#[derive(Deserialize, Default)]
pub struct CustomerReq {
    pub name: Option<String>,
    pub tax_id: Option<String>,
    pub branch: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
}

#[derive(Deserialize)]
pub struct SaleReq {
    pub items: Vec<LineReq>,
    pub discount_cents: Option<i64>,
    pub payments: Vec<Payment>,
    pub note: Option<String>,
    pub customer: Option<CustomerReq>,
    /// Issue a full tax invoice immediately (requires customer name + address).
    pub issue_invoice: Option<bool>,
}

#[derive(FromRow)]
struct LockedProduct {
    id: Uuid,
    shop_id: Uuid,
    name: String,
    sku: String,
    price_cents: i64,
    stock: i32,
    status: String,
}

pub async fn create_sale(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<SaleReq>,
) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    if req.items.is_empty() {
        return Err(AppError::bad("add at least one item"));
    }
    if req.items.len() > MAX_LINES {
        return Err(AppError::bad(format!("at most {MAX_LINES} lines per sale")));
    }

    let mut tx = st.db.begin().await?;

    // Lock every referenced product once (sorted → no deadlocks between tills).
    let mut ids: Vec<Uuid> = req.items.iter().filter_map(|l| l.product_id).collect();
    ids.sort();
    ids.dedup();
    let locked: Vec<LockedProduct> = sqlx::query_as(
        "SELECT id, shop_id, name, sku, price_cents, stock, status FROM products
         WHERE id = ANY($1) ORDER BY id FOR UPDATE",
    )
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    let products: BTreeMap<Uuid, LockedProduct> = locked.into_iter().map(|p| (p.id, p)).collect();

    struct Line {
        product_id: Option<Uuid>,
        name: String,
        sku: String,
        qty: i32,
        unit: i64,
        discount: i64,
        total: i64,
    }
    let mut lines = Vec::new();
    let mut need: BTreeMap<Uuid, i32> = BTreeMap::new();
    for l in &req.items {
        if !(1..=10_000).contains(&l.qty) {
            return Err(AppError::bad("qty must be between 1 and 10000"));
        }
        let (name, sku, unit) = match l.product_id {
            Some(pid) => {
                let p = products.get(&pid).ok_or_else(|| AppError::bad("product not found"))?;
                if p.shop_id != shop_id {
                    return Err(AppError::bad(format!("'{}' belongs to another shop", p.name)));
                }
                if p.status == "archived" {
                    return Err(AppError::bad(format!("'{}' is archived", p.name)));
                }
                *need.entry(pid).or_default() += l.qty;
                (p.name.clone(), p.sku.clone(), l.unit_price_cents.unwrap_or(p.price_cents))
            }
            None => {
                let name = l.name.as_deref().map(str::trim).filter(|n| !n.is_empty())
                    .ok_or_else(|| AppError::bad("custom items need a name"))?;
                let unit = l.unit_price_cents.ok_or_else(|| AppError::bad("custom items need a price"))?;
                (name.chars().take(120).collect(), String::new(), unit)
            }
        };
        if unit < 0 {
            return Err(AppError::bad("price cannot be negative"));
        }
        let gross = unit * l.qty as i64;
        let discount = l.discount_cents.unwrap_or(0);
        if discount < 0 || discount > gross {
            return Err(AppError::bad(format!("discount on '{name}' is larger than the line")));
        }
        lines.push(Line { product_id: l.product_id, name, sku, qty: l.qty, unit, discount, total: gross - discount });
    }
    for (pid, qty) in &need {
        let p = &products[pid];
        if p.stock < *qty {
            return Err(AppError::bad(format!("only {} × '{}' in stock", p.stock, p.name)));
        }
    }

    let subtotal: i64 = lines.iter().map(|l| l.total).sum();
    let bill_discount = req.discount_cents.unwrap_or(0);
    if bill_discount < 0 || bill_discount > subtotal {
        return Err(AppError::bad("bill discount is larger than the subtotal"));
    }
    let net = subtotal - bill_discount;
    let (vat, total) = vat_for(net, shop.vat_bps, shop.prices_include_vat);

    // Payments: change can only come from cash.
    if req.payments.is_empty() && total > 0 {
        return Err(AppError::bad("add a payment"));
    }
    let mut paid = 0i64;
    let mut cash = 0i64;
    for p in &req.payments {
        if !METHODS.contains(&p.method.as_str()) {
            return Err(AppError::bad(format!("payment method must be one of {METHODS:?}")));
        }
        if p.amount_cents <= 0 {
            return Err(AppError::bad("payment amounts must be positive"));
        }
        paid += p.amount_cents;
        if p.method == "cash" {
            cash += p.amount_cents;
        }
    }
    if paid < total {
        return Err(AppError::bad(format!("payment is short by {}", total - paid)));
    }
    let change = paid - total;
    if change > cash {
        return Err(AppError::bad("card/transfer/QR payments cannot exceed the amount due"));
    }

    let customer = req.customer.unwrap_or_default();
    let clean = |s: Option<String>| s.map(|v| v.trim().to_string()).unwrap_or_default();
    let (c_name, c_tax, c_branch, c_addr, c_phone) = (
        clean(customer.name),
        clean(customer.tax_id),
        clean(customer.branch),
        clean(customer.address),
        clean(customer.phone),
    );
    let issue_invoice = req.issue_invoice.unwrap_or(false);
    if issue_invoice && (c_name.is_empty() || c_addr.is_empty()) {
        return Err(AppError::bad("a tax invoice needs the customer's name and address"));
    }

    let number = next_number(&mut tx, shop_id, "receipt", &shop.receipt_prefix).await?;
    let invoice_number = if issue_invoice {
        Some(next_number(&mut tx, shop_id, "invoice", &shop.invoice_prefix).await?)
    } else {
        None
    };
    let sale_id: Uuid = sqlx::query_scalar(
        "INSERT INTO pos_sales (shop_id, number, cashier_id, subtotal_cents, discount_cents, vat_bps,
             prices_include_vat, vat_cents, total_cents, paid_cents, change_cents, payments, currency, note,
             invoice_number, invoice_issued_at, customer_name, customer_tax_id, customer_branch,
             customer_address, customer_phone)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,
                 CASE WHEN $15::text IS NULL THEN NULL ELSE now() END,$16,$17,$18,$19,$20)
         RETURNING id",
    )
    .bind(shop_id)
    .bind(&number)
    .bind(user.id)
    .bind(subtotal)
    .bind(bill_discount)
    .bind(shop.vat_bps)
    .bind(shop.prices_include_vat)
    .bind(vat)
    .bind(total)
    .bind(paid)
    .bind(change)
    .bind(serde_json::to_value(&req.payments).unwrap())
    .bind(&shop.currency)
    .bind(req.note.unwrap_or_default().chars().take(500).collect::<String>())
    .bind(&invoice_number)
    .bind(&c_name)
    .bind(&c_tax)
    .bind(&c_branch)
    .bind(&c_addr)
    .bind(&c_phone)
    .fetch_one(&mut *tx)
    .await?;

    for (i, l) in lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO pos_sale_items (sale_id, product_id, name, sku, qty, unit_price_cents, discount_cents, line_total_cents, position)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(sale_id)
        .bind(l.product_id)
        .bind(&l.name)
        .bind(&l.sku)
        .bind(l.qty)
        .bind(l.unit)
        .bind(l.discount)
        .bind(l.total)
        .bind(i as i32)
        .execute(&mut *tx)
        .await?;
    }
    for (pid, qty) in &need {
        move_stock(&mut tx, *pid, -qty, "sale", sale_id, user.id, &format!("POS {number}")).await?;
    }
    tx.commit().await?;
    load_doc(&st, sale_id).await.map(Json)
}

async fn move_stock(
    tx: &mut Transaction<'_, Postgres>,
    product_id: Uuid,
    delta: i32,
    reason: &str,
    sale_id: Uuid,
    by: Uuid,
    note: &str,
) -> AppResult<()> {
    let after: i32 = sqlx::query_scalar("UPDATE products SET stock = stock + $2, updated_at = now() WHERE id = $1 RETURNING stock")
        .bind(product_id)
        .bind(delta)
        .fetch_one(&mut **tx)
        .await?;
    sqlx::query(
        "INSERT INTO inventory_movements (product_id, delta, stock_after, reason, ref_pos_sale_id, note, created_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7)",
    )
    .bind(product_id)
    .bind(delta)
    .bind(after)
    .bind(reason)
    .bind(sale_id)
    .bind(note)
    .bind(by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

/// Everything needed to print a receipt or invoice.
async fn load_doc(st: &AppState, id: Uuid) -> AppResult<Value> {
    let sale: PosSale = sqlx::query_as("SELECT * FROM pos_sales WHERE id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let items: Vec<PosItem> = sqlx::query_as(
        "SELECT id, product_id, name, sku, qty, unit_price_cents, discount_cents, line_total_cents
         FROM pos_sale_items WHERE sale_id = $1 ORDER BY position",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1").bind(sale.shop_id).fetch_one(&st.db).await?;
    let cashier: Option<String> = match sale.cashier_id {
        Some(c) => sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1").bind(c).fetch_optional(&st.db).await?,
        None => None,
    };
    let net = sale.subtotal_cents - sale.discount_cents;
    let before_vat = if sale.prices_include_vat { net - sale.vat_cents } else { net };
    Ok(json!({
        "sale": sale,
        "items": items,
        "cashier": cashier,
        "amount_before_vat_cents": before_vat,
        "shop": {
            "id": shop.id, "name": shop.name, "slug": shop.slug, "logo_url": shop.logo_url,
            "legal_name": shop.legal_name, "tax_id": shop.tax_id, "branch": shop.branch,
            "address": shop.address, "phone": shop.phone, "currency": shop.currency,
            "receipt_footer": shop.receipt_footer
        }
    }))
}

async fn sale_shop(st: &AppState, id: Uuid, user: &AuthUser) -> AppResult<Shop> {
    let shop_id: Uuid = sqlx::query_scalar("SELECT shop_id FROM pos_sales WHERE id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(st, shop_id, user).await
}

pub async fn get_sale(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    sale_shop(&st, id, &user).await?;
    load_doc(&st, id).await.map(Json)
}

#[derive(Deserialize)]
pub struct ListQ {
    /// Local date YYYY-MM-DD (in `tz`, default Asia/Bangkok).
    pub date: Option<NaiveDate>,
    pub tz: Option<String>,
    pub q: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

fn tz_or_default(tz: &Option<String>) -> String {
    tz.as_deref()
        .filter(|t| t.len() < 64 && t.chars().all(|c| c.is_ascii_alphanumeric() || "/_+-".contains(c)))
        .unwrap_or("Asia/Bangkok")
        .to_string()
}

pub async fn list_sales(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<ListQ>,
) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, shop_id, &user).await?;
    type Row = (Uuid, String, String, i64, i64, Value, Option<String>, String, DateTime<Utc>, i64, Option<String>);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT s.id, s.number, s.status, s.total_cents, s.vat_cents, s.payments, s.invoice_number,
                s.customer_name, s.created_at,
                (SELECT COALESCE(SUM(qty),0)::bigint FROM pos_sale_items WHERE sale_id = s.id),
                u.display_name
         FROM pos_sales s LEFT JOIN users u ON u.id = s.cashier_id
         WHERE s.shop_id = $1
           AND ($2::date IS NULL OR (s.created_at AT TIME ZONE $3)::date = $2)
           AND ($4::text IS NULL OR s.number ILIKE '%'||$4||'%' OR s.invoice_number ILIKE '%'||$4||'%'
                OR s.customer_name ILIKE '%'||$4||'%')
           AND ($5::text IS NULL OR s.status = $5)
         ORDER BY s.created_at DESC LIMIT $6 OFFSET $7",
    )
    .bind(shop_id)
    .bind(q.date)
    .bind(tz_or_default(&q.tz))
    .bind(q.q.filter(|s| !s.trim().is_empty()))
    .bind(q.status.filter(|s| !s.is_empty()))
    .bind(q.limit.unwrap_or(100).clamp(1, 500))
    .bind(q.offset.unwrap_or(0).max(0))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, number, status, total, vat, payments, inv, customer, at, units, cashier)| {
                json!({ "id": id, "number": number, "status": status, "total_cents": total, "vat_cents": vat,
                        "payments": payments, "invoice_number": inv, "customer_name": customer,
                        "created_at": at, "units": units, "cashier": cashier })
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct VoidReq {
    pub reason: String,
}

pub async fn void_sale(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<VoidReq>,
) -> AppResult<Json<Value>> {
    sale_shop(&st, id, &user).await?;
    if req.reason.trim().is_empty() {
        return Err(AppError::bad("give a reason for voiding"));
    }
    let mut tx = st.db.begin().await?;
    let number: Option<String> = sqlx::query_scalar(
        "UPDATE pos_sales SET status='voided', void_reason=$2, voided_at=now(), voided_by=$3
         WHERE id=$1 AND status='completed' RETURNING number",
    )
    .bind(id)
    .bind(req.reason.trim())
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;
    let number = number.ok_or_else(|| AppError::bad("sale is already voided"))?;
    let items: Vec<(Uuid, i64)> = sqlx::query_as(
        "SELECT product_id, SUM(qty)::bigint FROM pos_sale_items WHERE sale_id=$1 AND product_id IS NOT NULL GROUP BY product_id",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    for (pid, qty) in items {
        move_stock(&mut tx, pid, qty as i32, "cancel", id, user.id, &format!("Void POS {number}")).await?;
    }
    tx.commit().await?;
    load_doc(&st, id).await.map(Json)
}

#[derive(Deserialize)]
pub struct InvoiceReq {
    pub customer_name: String,
    pub customer_address: String,
    pub customer_tax_id: Option<String>,
    pub customer_branch: Option<String>,
    pub customer_phone: Option<String>,
}

/// Issue a full tax invoice for an existing sale (once; the number is permanent).
pub async fn issue_invoice(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<InvoiceReq>,
) -> AppResult<Json<Value>> {
    let shop = sale_shop(&st, id, &user).await?;
    if req.customer_name.trim().is_empty() || req.customer_address.trim().is_empty() {
        return Err(AppError::bad("a tax invoice needs the customer's name and address"));
    }
    let mut tx = st.db.begin().await?;
    let (status, existing): (String, Option<String>) =
        sqlx::query_as("SELECT status, invoice_number FROM pos_sales WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if status != "completed" {
        return Err(AppError::bad("cannot invoice a voided sale"));
    }
    if existing.is_some() {
        return Err(AppError::Conflict("a tax invoice was already issued for this sale — reprint it instead".into()));
    }
    let number = next_number(&mut tx, shop.id, "invoice", &shop.invoice_prefix).await?;
    sqlx::query(
        "UPDATE pos_sales SET invoice_number=$2, invoice_issued_at=now(), customer_name=$3, customer_address=$4,
             customer_tax_id=$5, customer_branch=$6, customer_phone=$7
         WHERE id=$1",
    )
    .bind(id)
    .bind(&number)
    .bind(req.customer_name.trim())
    .bind(req.customer_address.trim())
    .bind(req.customer_tax_id.unwrap_or_default().trim())
    .bind(req.customer_branch.unwrap_or_default().trim())
    .bind(req.customer_phone.unwrap_or_default().trim())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    load_doc(&st, id).await.map(Json)
}

#[derive(Deserialize)]
pub struct SummaryQ {
    pub date: Option<NaiveDate>,
    pub tz: Option<String>,
}

/// End-of-day (Z) report.
pub async fn summary(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<SummaryQ>,
) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    let tz = tz_or_default(&q.tz);
    let date: NaiveDate = match q.date {
        Some(d) => d,
        None => sqlx::query_scalar("SELECT (now() AT TIME ZONE $1)::date").bind(&tz).fetch_one(&st.db).await?,
    };
    const DAY: &str = "shop_id = $1 AND (created_at AT TIME ZONE $2)::date = $3";

    let (count, gross, discounts, vat, total, first, last): (i64, i64, i64, i64, i64, Option<String>, Option<String>) =
        sqlx::query_as(&format!(
            "SELECT COUNT(*), COALESCE(SUM(subtotal_cents),0)::bigint, COALESCE(SUM(discount_cents),0)::bigint,
                    COALESCE(SUM(vat_cents),0)::bigint, COALESCE(SUM(total_cents),0)::bigint,
                    MIN(number), MAX(number)
             FROM pos_sales WHERE {DAY} AND status = 'completed'"
        ))
        .bind(shop_id)
        .bind(&tz)
        .bind(date)
        .fetch_one(&st.db)
        .await?;
    let (voids, void_total): (i64, i64) = sqlx::query_as(&format!(
        "SELECT COUNT(*), COALESCE(SUM(total_cents),0)::bigint FROM pos_sales WHERE {DAY} AND status = 'voided'"
    ))
    .bind(shop_id)
    .bind(&tz)
    .bind(date)
    .fetch_one(&st.db)
    .await?;
    // Takings by method; cash is net of change given.
    let methods: Vec<(String, i64)> = sqlx::query_as(&format!(
        "SELECT p->>'method', SUM((p->>'amount_cents')::bigint)::bigint
         FROM pos_sales, jsonb_array_elements(payments) p
         WHERE {DAY} AND status = 'completed' GROUP BY 1"
    ))
    .bind(shop_id)
    .bind(&tz)
    .bind(date)
    .fetch_all(&st.db)
    .await?;
    let change: i64 = sqlx::query_scalar(&format!(
        "SELECT COALESCE(SUM(change_cents),0)::bigint FROM pos_sales WHERE {DAY} AND status = 'completed'"
    ))
    .bind(shop_id)
    .bind(&tz)
    .bind(date)
    .fetch_one(&st.db)
    .await?;
    let mut by_method = serde_json::Map::new();
    for m in METHODS {
        by_method.insert(m.into(), json!(0));
    }
    for (m, amt) in methods {
        let v = if m == "cash" { amt - change } else { amt };
        by_method.insert(m, json!(v));
    }
    let top: Vec<(String, i64, i64)> = sqlx::query_as(&format!(
        "SELECT i.name, SUM(i.qty)::bigint, SUM(i.line_total_cents)::bigint
         FROM pos_sale_items i JOIN pos_sales s ON s.id = i.sale_id
         WHERE s.{} AND s.status = 'completed'
         GROUP BY i.name ORDER BY 3 DESC LIMIT 10",
        DAY.replace("created_at", "s.created_at")
    ))
    .bind(shop_id)
    .bind(&tz)
    .bind(date)
    .fetch_all(&st.db)
    .await?;
    let invoices: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM pos_sales WHERE {DAY} AND invoice_number IS NOT NULL AND status='completed'"
    ))
    .bind(shop_id)
    .bind(&tz)
    .bind(date)
    .fetch_one(&st.db)
    .await?;

    Ok(Json(json!({
        "date": date, "tz": tz, "generated_at": Utc::now(),
        "shop": { "name": shop.name, "legal_name": shop.legal_name, "tax_id": shop.tax_id, "branch": shop.branch,
                  "address": shop.address, "currency": shop.currency },
        "sales": count, "gross_cents": gross, "discount_cents": discounts, "vat_cents": vat,
        "net_cents": total - vat, "total_cents": total,
        "first_number": first, "last_number": last, "invoices": invoices,
        "voids": voids, "void_total_cents": void_total, "change_given_cents": change,
        "by_method": by_method,
        "top_items": top.into_iter().map(|(n, q, t)| json!({ "name": n, "qty": q, "total_cents": t })).collect::<Vec<_>>()
    })))
}

// ---------------------------------------------------------------------------
// Online order documents (invoice / packing slip)
// ---------------------------------------------------------------------------

/// Printable data for an online order: visible to the buyer and the supplier shop owner.
pub async fn order_document(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let (buyer_id, supplier): (Uuid, Uuid) = sqlx::query_as(
        "SELECT o.buyer_id, (SELECT supplier_shop_id FROM order_items WHERE order_id = o.id LIMIT 1)
         FROM orders o WHERE o.id = $1",
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if buyer_id != user.id && owned_shop(&st, supplier, &user).await.is_err() {
        return Err(AppError::Forbidden);
    }
    let order: crate::models::Order = sqlx::query_as("SELECT * FROM orders WHERE id = $1").bind(id).fetch_one(&st.db).await?;
    let items: Vec<crate::models::OrderItem> = sqlx::query_as("SELECT * FROM order_items WHERE order_id = $1").bind(id).fetch_all(&st.db).await?;
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1").bind(supplier).fetch_one(&st.db).await?;
    let (buyer_name, buyer_email): (String, String) =
        sqlx::query_as("SELECT display_name, email FROM users WHERE id = $1").bind(buyer_id).fetch_one(&st.db).await?;
    let (vat, _) = vat_for(order.total_cents, shop.vat_bps, true);
    Ok(Json(json!({
        "order": order, "items": items,
        "buyer": { "name": buyer_name, "email": buyer_email },
        "vat_bps": shop.vat_bps, "vat_cents": vat,
        "shop": {
            "name": shop.name, "legal_name": shop.legal_name, "tax_id": shop.tax_id, "branch": shop.branch,
            "address": shop.address, "phone": shop.phone, "currency": shop.currency, "logo_url": shop.logo_url,
            "receipt_footer": shop.receipt_footer
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vat_math() {
        // 107.00 incl. 7% → VAT 7.00
        assert_eq!(vat_for(10_700, 700, true), (700, 10_700));
        // 100.00 excl. 7% → VAT 7.00, total 107.00
        assert_eq!(vat_for(10_000, 700, false), (700, 10_700));
        // rounding half-up: 0.50 incl 7% = 0.0327 → 0.03
        assert_eq!(vat_for(50, 700, true), (3, 50));
        assert_eq!(vat_for(999, 0, true), (0, 999));
    }
}
