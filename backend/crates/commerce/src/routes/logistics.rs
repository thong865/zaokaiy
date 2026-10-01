//! Delivery couriers (Anousith, HAL, Mixay, shop delivery …), per-shop shipping terms, checkout
//! quotes, and the cash-on-delivery (COD) ledger.
//!
//! Lao couriers have no public booking API: the seller hands the parcel to the courier and types the
//! tracking number. COD money moves customer → courier → seller, so each COD order walks
//! `pending → collected → remitted` (or `returned` when the customer refuses the parcel).

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::{
    auth::{AdminUser, AuthUser},
    error::{AppError, AppResult},
    routes::owned_shop,
    AppState,
};

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct Carrier {
    pub code: String,
    pub name: String,
    pub name_lo: String,
    pub website: String,
    pub tracking_url: String,
    pub phone: String,
    pub supports_cod: bool,
    pub active: bool,
    pub sort: i32,
    pub created_at: DateTime<Utc>,
}

/// Public list of active couriers (names, websites, tracking link templates).
pub async fn carriers(State(st): State<AppState>) -> AppResult<Json<Vec<Carrier>>> {
    Ok(Json(
        sqlx::query_as("SELECT * FROM carriers WHERE active ORDER BY sort, name").fetch_all(&st.db).await?,
    ))
}

// ---------------------------------------------------------------------------------------------
// Admin: manage the courier list
// ---------------------------------------------------------------------------------------------

pub async fn admin_carriers(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Vec<Carrier>>> {
    Ok(Json(sqlx::query_as("SELECT * FROM carriers ORDER BY sort, name").fetch_all(&st.db).await?))
}

#[derive(Deserialize)]
pub struct CarrierReq {
    pub code: Option<String>,
    pub name: Option<String>,
    pub name_lo: Option<String>,
    pub website: Option<String>,
    pub tracking_url: Option<String>,
    pub phone: Option<String>,
    pub supports_cod: Option<bool>,
    pub active: Option<bool>,
    pub sort: Option<i32>,
}

fn check_url(u: &Option<String>) -> AppResult<()> {
    match u.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() && !(s.starts_with("https://") || s.starts_with("http://")) => {
            Err(AppError::bad("links must start with https://"))
        }
        _ => Ok(()),
    }
}

pub async fn admin_create_carrier(State(st): State<AppState>, _a: AdminUser, Json(r): Json<CarrierReq>) -> AppResult<Json<Carrier>> {
    let code = r.code.as_deref().unwrap_or_default().trim().to_lowercase();
    if !(2..=30).contains(&code.len()) || !code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(AppError::bad("code must be 2-30 lowercase letters, digits or _"));
    }
    let name = r.name.as_deref().unwrap_or_default().trim().to_string();
    if name.is_empty() {
        return Err(AppError::bad("name is required"));
    }
    check_url(&r.website)?;
    check_url(&r.tracking_url)?;
    let c: Carrier = sqlx::query_as(
        "INSERT INTO carriers (code, name, name_lo, website, tracking_url, phone, supports_cod, active, sort)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING *",
    )
    .bind(&code)
    .bind(&name)
    .bind(r.name_lo.unwrap_or_default().trim())
    .bind(r.website.unwrap_or_default().trim())
    .bind(r.tracking_url.unwrap_or_default().trim())
    .bind(r.phone.unwrap_or_default().trim())
    .bind(r.supports_cod.unwrap_or(true))
    .bind(r.active.unwrap_or(true))
    .bind(r.sort.unwrap_or(50))
    .fetch_one(&st.db)
    .await
    .map_err(|e| match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict("a courier with this code already exists".into()),
        o => o,
    })?;
    Ok(Json(c))
}

pub async fn admin_update_carrier(
    State(st): State<AppState>,
    _a: AdminUser,
    Path(code): Path<String>,
    Json(r): Json<CarrierReq>,
) -> AppResult<Json<Carrier>> {
    check_url(&r.website)?;
    check_url(&r.tracking_url)?;
    if r.name.as_deref().is_some_and(|n| n.trim().is_empty()) {
        return Err(AppError::bad("name is required"));
    }
    let c: Option<Carrier> = sqlx::query_as(
        "UPDATE carriers SET
            name = COALESCE($2, name), name_lo = COALESCE($3, name_lo), website = COALESCE($4, website),
            tracking_url = COALESCE($5, tracking_url), phone = COALESCE($6, phone),
            supports_cod = COALESCE($7, supports_cod), active = COALESCE($8, active), sort = COALESCE($9, sort)
         WHERE code = $1 RETURNING *",
    )
    .bind(&code)
    .bind(r.name.map(|s| s.trim().to_string()))
    .bind(r.name_lo.map(|s| s.trim().to_string()))
    .bind(r.website.map(|s| s.trim().to_string()))
    .bind(r.tracking_url.map(|s| s.trim().to_string()))
    .bind(r.phone.map(|s| s.trim().to_string()))
    .bind(r.supports_cod)
    .bind(r.active)
    .bind(r.sort)
    .fetch_optional(&st.db)
    .await?;
    c.map(Json).ok_or(AppError::NotFound)
}

// ---------------------------------------------------------------------------------------------
// Seller: shipping terms per courier
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Serialize, FromRow)]
pub struct ShippingOption {
    pub carrier_code: String,
    pub name: String,
    pub name_lo: String,
    pub website: String,
    pub tracking_url: String,
    pub supports_cod: bool,
    pub enabled: bool,
    pub fee_payer: String,
    pub fee_cents: i64,
    pub home_fee_cents: Option<i64>,
    pub free_over_cents: Option<i64>,
    pub cod_enabled: bool,
    pub cod_fee_cents: i64,
    pub eta: String,
    pub note: String,
    /// The shop has saved terms for this courier (false = defaults shown for an unused courier).
    pub configured: bool,
}

const OPTION_COLS: &str = "c.code AS carrier_code, c.name, c.name_lo, c.website, c.tracking_url, c.supports_cod,
    COALESCE(ss.enabled, false) AS enabled, COALESCE(ss.fee_payer, 'buyer') AS fee_payer,
    COALESCE(ss.fee_cents, 0) AS fee_cents, ss.home_fee_cents, ss.free_over_cents,
    COALESCE(ss.cod_enabled, false) AND c.supports_cod AS cod_enabled, COALESCE(ss.cod_fee_cents, 0) AS cod_fee_cents,
    COALESCE(ss.eta, '') AS eta, COALESCE(ss.note, '') AS note, ss.shop_id IS NOT NULL AS configured";

/// All active couriers with this shop's terms (unconfigured ones come back disabled).
pub async fn shop_shipping(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Vec<ShippingOption>>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows: Vec<ShippingOption> = sqlx::query_as(&format!(
        "SELECT {OPTION_COLS} FROM carriers c
         LEFT JOIN shop_shipping ss ON ss.carrier_code = c.code AND ss.shop_id = $1
         WHERE c.active OR ss.enabled ORDER BY COALESCE(ss.sort, c.sort), c.sort, c.name"
    ))
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct ShippingReq {
    pub enabled: bool,
    pub fee_payer: String,
    pub fee_cents: i64,
    pub home_fee_cents: Option<i64>,
    pub free_over_cents: Option<i64>,
    #[serde(default)]
    pub cod_enabled: bool,
    #[serde(default)]
    pub cod_fee_cents: i64,
    #[serde(default)]
    pub eta: String,
    #[serde(default)]
    pub note: String,
}

pub async fn set_shop_shipping(
    State(st): State<AppState>,
    user: AuthUser,
    Path((shop_id, code)): Path<(Uuid, String)>,
    Json(r): Json<ShippingReq>,
) -> AppResult<Json<Vec<ShippingOption>>> {
    owned_shop(&st, shop_id, &user).await?;
    if !matches!(r.fee_payer.as_str(), "buyer" | "destination" | "seller") {
        return Err(AppError::bad("fee_payer must be buyer, destination or seller"));
    }
    if r.fee_cents < 0 || r.cod_fee_cents < 0 || r.home_fee_cents.is_some_and(|f| f < 0) || r.free_over_cents.is_some_and(|f| f <= 0) {
        return Err(AppError::bad("fees cannot be negative"));
    }
    let supports_cod: Option<bool> = sqlx::query_scalar("SELECT supports_cod FROM carriers WHERE code = $1").bind(&code).fetch_optional(&st.db).await?;
    let supports_cod = supports_cod.ok_or(AppError::NotFound)?;
    if r.cod_enabled && !supports_cod {
        return Err(AppError::bad("this courier does not collect cash on delivery"));
    }
    sqlx::query(
        "INSERT INTO shop_shipping (shop_id, carrier_code, enabled, fee_payer, fee_cents, home_fee_cents, free_over_cents,
                                    cod_enabled, cod_fee_cents, eta, note)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
         ON CONFLICT (shop_id, carrier_code) DO UPDATE SET enabled=$3, fee_payer=$4, fee_cents=$5, home_fee_cents=$6,
             free_over_cents=$7, cod_enabled=$8, cod_fee_cents=$9, eta=$10, note=$11",
    )
    .bind(shop_id)
    .bind(&code)
    .bind(r.enabled)
    .bind(&r.fee_payer)
    .bind(r.fee_cents)
    .bind(r.home_fee_cents)
    .bind(r.free_over_cents)
    .bind(r.cod_enabled)
    .bind(r.cod_fee_cents)
    .bind(r.eta.trim().chars().take(60).collect::<String>())
    .bind(r.note.trim().chars().take(200).collect::<String>())
    .execute(&st.db)
    .await?;
    shop_shipping(State(st), user, Path(shop_id)).await
}

#[derive(Deserialize)]
pub struct OptionsQ {
    /// Comma-separated shop ids.
    pub shops: String,
}

/// Enabled delivery options per shop, for the cart and the social checkout page.
pub async fn public_options(State(st): State<AppState>, Query(q): Query<OptionsQ>) -> AppResult<Json<Value>> {
    let ids: Vec<Uuid> = q.shops.split(',').filter_map(|s| s.trim().parse().ok()).take(50).collect();
    let rows: Vec<(Uuid, sqlx::types::Json<Value>)> = sqlx::query_as(&format!(
        "SELECT ss.shop_id, to_jsonb(x) FROM shop_shipping ss JOIN carriers c ON c.code = ss.carrier_code,
             LATERAL (SELECT {OPTION_COLS}) x
         WHERE ss.shop_id = ANY($1) AND ss.enabled AND c.active ORDER BY ss.sort, c.sort, c.name"
    ))
    .bind(&ids)
    .fetch_all(&st.db)
    .await?;
    let mut out = serde_json::Map::new();
    for id in &ids {
        out.insert(id.to_string(), json!([]));
    }
    for (shop, opt) in rows {
        if let Some(Value::Array(a)) = out.get_mut(&shop.to_string()) {
            a.push(opt.0);
        }
    }
    Ok(Json(Value::Object(out)))
}

/// Enabled delivery options of one shop (social checkout page).
pub async fn options_for(db: &sqlx::PgPool, shop_id: Uuid) -> AppResult<Vec<ShippingOption>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {OPTION_COLS} FROM shop_shipping ss JOIN carriers c ON c.code = ss.carrier_code
         WHERE ss.shop_id = $1 AND ss.enabled AND c.active ORDER BY ss.sort, c.sort, c.name"
    ))
    .bind(shop_id)
    .fetch_all(db)
    .await?)
}

pub async fn shop_has_shipping(conn: &mut PgConnection, shop_id: Uuid) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop_shipping ss JOIN carriers c ON c.code = ss.carrier_code WHERE ss.shop_id = $1 AND ss.enabled AND c.active)",
    )
    .bind(shop_id)
    .fetch_one(conn)
    .await?)
}

/// Price of one delivery choice.
#[derive(Debug, Clone)]
pub struct Quote {
    pub carrier_code: String,
    pub delivery_type: String,
    /// Who pays the courier fee after free-shipping rules: buyer | destination | seller.
    pub fee_payer: String,
    /// The courier fee for this parcel (recorded even when the buyer does not pay it now).
    pub fee_cents: i64,
    /// Part of the fee added to what the buyer pays at checkout / on delivery.
    pub charged_fee_cents: i64,
    pub cod: bool,
    pub cod_fee_cents: i64,
}

pub async fn quote(
    conn: &mut PgConnection,
    shop_id: Uuid,
    carrier: &str,
    delivery_type: &str,
    cod: bool,
    subtotal_cents: i64,
) -> AppResult<Quote> {
    if !matches!(delivery_type, "branch" | "home") {
        return Err(AppError::bad("delivery must be branch pickup or home delivery"));
    }
    #[derive(FromRow)]
    struct Row {
        fee_payer: String,
        fee_cents: i64,
        home_fee_cents: Option<i64>,
        free_over_cents: Option<i64>,
        cod_enabled: bool,
        cod_fee_cents: i64,
        supports_cod: bool,
    }
    let row: Option<Row> = sqlx::query_as(
        "SELECT ss.fee_payer, ss.fee_cents, ss.home_fee_cents, ss.free_over_cents, ss.cod_enabled, ss.cod_fee_cents, c.supports_cod
         FROM shop_shipping ss JOIN carriers c ON c.code = ss.carrier_code
         WHERE ss.shop_id = $1 AND ss.carrier_code = $2 AND ss.enabled AND c.active",
    )
    .bind(shop_id)
    .bind(carrier)
    .fetch_optional(&mut *conn)
    .await?;
    let r = row.ok_or_else(|| AppError::bad("this delivery option is not available"))?;
    let fee = if delivery_type == "home" {
        r.home_fee_cents.ok_or_else(|| AppError::bad("home delivery is not offered with this courier"))?
    } else {
        r.fee_cents
    };
    if cod && !(r.cod_enabled && r.supports_cod) {
        return Err(AppError::bad("cash on delivery is not available with this courier"));
    }
    let payer = if r.free_over_cents.is_some_and(|f| subtotal_cents >= f) { "seller".to_string() } else { r.fee_payer };
    Ok(Quote {
        carrier_code: carrier.to_string(),
        delivery_type: delivery_type.to_string(),
        charged_fee_cents: if payer == "buyer" { fee } else { 0 },
        fee_payer: payer,
        fee_cents: fee,
        cod,
        cod_fee_cents: if cod { r.cod_fee_cents } else { 0 },
    })
}

// ---------------------------------------------------------------------------------------------
// COD ledger (marketplace orders + comment/chat orders)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Serialize, FromRow)]
pub struct CodRow {
    pub kind: String,
    pub id: Uuid,
    pub number: String,
    pub customer: String,
    pub phone: String,
    pub carrier_code: Option<String>,
    pub tracking_no: String,
    pub cod_amount_cents: i64,
    pub cod_status: String,
    pub cod_remit_ref: String,
    pub order_status: String,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub cod_collected_at: Option<DateTime<Utc>>,
    pub cod_remitted_at: Option<DateTime<Utc>>,
}

const COD_UNION: &str = "
    SELECT 'order' AS kind, o.id, upper(left(o.id::text, 8)) AS number,
           COALESCE(o.shipping_address->>'name', u.display_name) AS customer, COALESCE(o.shipping_address->>'phone', '') AS phone,
           o.carrier_code, o.tracking_no, o.cod_amount_cents, o.cod_status, o.cod_remit_ref, o.status AS order_status,
           o.currency, o.created_at, o.shipped_at, o.cod_collected_at, o.cod_remitted_at,
           (SELECT oi.supplier_shop_id FROM order_items oi WHERE oi.order_id = o.id LIMIT 1) AS shop_id
    FROM orders o JOIN users u ON u.id = o.buyer_id WHERE o.cod_status <> 'none'
    UNION ALL
    SELECT 'social', s.id, s.number, s.ship_name, s.ship_phone, s.carrier_code, s.tracking_no, s.cod_amount_cents, s.cod_status,
           s.cod_remit_ref, s.status, s.currency, s.created_at, s.shipped_at, s.cod_collected_at, s.cod_remitted_at, s.shop_id
    FROM social_orders s WHERE s.cod_status <> 'none'";

#[derive(Deserialize)]
pub struct CodQ {
    pub status: Option<String>,
    pub carrier: Option<String>,
}

pub async fn cod_ledger(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Query(q): Query<CodQ>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows: Vec<CodRow> = sqlx::query_as(&format!(
        "SELECT kind, id, number, customer, phone, carrier_code, tracking_no, cod_amount_cents, cod_status, cod_remit_ref,
                order_status, currency, created_at, shipped_at, cod_collected_at, cod_remitted_at
         FROM ({COD_UNION}) x
         WHERE shop_id = $1 AND ($2::text IS NULL OR cod_status = $2) AND ($3::text IS NULL OR carrier_code = $3)
         ORDER BY created_at DESC LIMIT 500"
    ))
    .bind(shop_id)
    .bind(q.status.filter(|s| !s.is_empty()))
    .bind(q.carrier.filter(|s| !s.is_empty()))
    .fetch_all(&st.db)
    .await?;
    // Totals per courier and status (all time, ignoring filters) — what the courier still owes the shop.
    let totals: Vec<(Option<String>, String, i64, i64)> = sqlx::query_as(&format!(
        "SELECT carrier_code, cod_status, count(*), COALESCE(sum(cod_amount_cents), 0)::bigint
         FROM ({COD_UNION}) x WHERE shop_id = $1 GROUP BY carrier_code, cod_status"
    ))
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    let totals: Vec<Value> = totals
        .into_iter()
        .map(|(carrier, status, n, cents)| json!({ "carrier_code": carrier, "cod_status": status, "count": n, "amount_cents": cents }))
        .collect();
    let templates: Vec<(String, String)> = sqlx::query_as("SELECT code, tracking_url FROM carriers").fetch_all(&st.db).await?;
    let rows: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            let link = r.carrier_code.as_ref().and_then(|c| templates.iter().find(|(k, _)| k == c)).and_then(|(_, t)| tracking_link(t, &r.tracking_no));
            let mut v = serde_json::to_value(&r).unwrap_or_default();
            v["tracking_link"] = json!(link);
            v
        })
        .collect();
    Ok(Json(json!({ "rows": rows, "totals": totals })))
}

#[derive(Deserialize)]
pub struct CodRef {
    pub kind: String,
    pub id: Uuid,
}

#[derive(Deserialize)]
pub struct CodUpdateReq {
    pub items: Vec<CodRef>,
    /// collected | remitted | returned
    pub status: String,
    /// Courier transfer / settlement reference (for remitted).
    pub reference: Option<String>,
}

/// Move COD orders along: collected (courier got the cash; order completes), remitted (courier
/// paid the shop), returned (customer refused; order cancelled and stock returned).
pub async fn cod_update(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<CodUpdateReq>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    if !matches!(r.status.as_str(), "collected" | "remitted" | "returned") {
        return Err(AppError::bad("status must be collected, remitted or returned"));
    }
    if r.items.is_empty() || r.items.len() > 200 {
        return Err(AppError::bad("select between 1 and 200 orders"));
    }
    let reference = r.reference.unwrap_or_default().trim().chars().take(120).collect::<String>();
    let mut tx = st.db.begin().await?;
    let mut updated = 0;
    for it in &r.items {
        let (cod, order_status, owner): (String, String, Option<Uuid>) = match it.kind.as_str() {
            "order" => sqlx::query_as(
                "SELECT o.cod_status, o.status, (SELECT supplier_shop_id FROM order_items WHERE order_id = o.id LIMIT 1)
                 FROM orders o WHERE o.id = $1 FOR UPDATE",
            ),
            "social" => sqlx::query_as("SELECT cod_status, status, shop_id FROM social_orders WHERE id = $1 FOR UPDATE"),
            _ => return Err(AppError::bad("kind must be order or social")),
        }
        .bind(it.id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
        if owner != Some(shop_id) {
            return Err(AppError::Forbidden);
        }
        let ok = matches!(
            (cod.as_str(), r.status.as_str()),
            ("pending", "collected") | ("pending" | "collected", "remitted") | ("pending", "returned")
        );
        if !ok {
            return Err(AppError::bad(format!("cannot mark a {cod} COD order as {}", r.status)));
        }
        if matches!(r.status.as_str(), "collected" | "remitted") && !matches!(order_status.as_str(), "shipped" | "completed") {
            return Err(AppError::bad("ship the order before recording the cash"));
        }
        if r.status == "returned" && order_status != "shipped" && order_status != "pending" && order_status != "confirmed" {
            return Err(AppError::bad(format!("cannot return an order that is {order_status}")));
        }
        let table = if it.kind == "order" { "orders" } else { "social_orders" };
        sqlx::query(&format!(
            "UPDATE {table} SET cod_status = $2, updated_at = now(),
                cod_collected_at = CASE WHEN $2 IN ('collected','remitted') THEN COALESCE(cod_collected_at, now()) ELSE cod_collected_at END,
                cod_remitted_at  = CASE WHEN $2 = 'remitted' THEN now() ELSE cod_remitted_at END,
                cod_remit_ref    = CASE WHEN $2 = 'remitted' THEN $3 ELSE cod_remit_ref END
             WHERE id = $1"
        ))
        .bind(it.id)
        .bind(&r.status)
        .bind(&reference)
        .execute(&mut *tx)
        .await?;
        match (it.kind.as_str(), r.status.as_str()) {
            // Cash collected = delivered: complete the order (approves resale commissions).
            ("order", "collected" | "remitted") if order_status == "shipped" => {
                super::orders::complete(&mut tx, it.id).await?;
            }
            ("social", "collected" | "remitted") if order_status == "shipped" => {
                sqlx::query("UPDATE social_orders SET status = 'completed', paid_at = COALESCE(paid_at, now()) WHERE id = $1")
                    .bind(it.id)
                    .execute(&mut *tx)
                    .await?;
                // Module hook (promo): loyalty points.
                crate::modules::promo::social_completed(&mut *tx, it.id).await?;
            }
            ("order", "returned") => super::orders::cancel_returned(&mut tx, it.id, user.id).await?,
            ("social", "returned") => super::social::cancel_returned(&mut tx, it.id).await?,
            _ => {}
        }
        updated += 1;
    }
    tx.commit().await?;
    Ok(Json(json!({ "updated": updated })))
}

/// `https://…/track?no={tracking}` → link for one parcel (None when the courier has no tracking page).
pub fn tracking_link(template: &str, tracking: &str) -> Option<String> {
    let t = tracking.trim();
    if template.is_empty() || t.is_empty() {
        return None;
    }
    let enc: String = t.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    Some(if template.contains("{tracking}") { template.replace("{tracking}", &enc) } else { template.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links() {
        assert_eq!(tracking_link("https://x.la/t?no={tracking}", " AB-12 ").as_deref(), Some("https://x.la/t?no=AB-12"));
        assert_eq!(tracking_link("https://x.la/t?no={tracking}", "a&b=c").as_deref(), Some("https://x.la/t?no=abc"));
        assert_eq!(tracking_link("https://x.la", "123").as_deref(), Some("https://x.la"));
        assert_eq!(tracking_link("", "123"), None);
        assert_eq!(tracking_link("https://x.la/{tracking}", ""), None);
    }
}
