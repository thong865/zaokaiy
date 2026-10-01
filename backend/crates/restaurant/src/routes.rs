use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use kernel::guards::{owned_shop, public_shop};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::{optional_user, AuthUser},
    error::{AppError, AppResult},
    AppState,
};

/// Kitchen flow. Any later step can be chosen; `cancelled` from anything not completed.
pub const FLOW: &[&str] = &["new", "accepted", "preparing", "ready", "served", "completed"];
pub const PAYMENT_METHODS: &[&str] = &["", "cash", "transfer", "card"];

fn token(n: usize) -> String {
    rand::thread_rng().sample_iter(&Alphanumeric).take(n).map(char::from).collect()
}

/// Shop-local date (Laos, UTC+7).
fn local_today() -> NaiveDate {
    (Utc::now() + chrono::Duration::hours(7)).date_naive()
}

fn clean_text(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

// ---------------------------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------------------------

#[derive(Serialize, FromRow, Clone)]
pub struct Settings {
    pub accepting_orders: bool,
    pub dine_in: bool,
    pub takeaway: bool,
    pub delivery: bool,
    pub service_charge_bps: i32,
    pub prep_minutes: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { accepting_orders: true, dine_in: true, takeaway: true, delivery: false, service_charge_bps: 0, prep_minutes: 15 }
    }
}

#[derive(Serialize, FromRow)]
pub struct Section {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub name: String,
    pub name_lo: String,
    pub position: i32,
}

#[derive(Serialize, FromRow)]
pub struct Item {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub section_id: Option<Uuid>,
    pub name: String,
    pub name_lo: String,
    pub description: String,
    pub price_cents: i64,
    pub image_url: String,
    pub spicy: i16,
    pub available: bool,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow)]
pub struct Table {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub label: String,
    pub seats: i32,
    pub token: String,
    pub active: bool,
}

#[derive(Serialize, FromRow)]
pub struct Order {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub number: i32,
    pub day: NaiveDate,
    pub mode: String,
    pub table_id: Option<Uuid>,
    pub table_label: String,
    pub user_id: Option<Uuid>,
    pub customer_name: String,
    pub customer_phone: String,
    pub address: String,
    pub note: String,
    pub status: String,
    pub subtotal_cents: i64,
    pub service_cents: i64,
    pub total_cents: i64,
    pub currency: String,
    pub paid: bool,
    pub payment_method: String,
    pub track_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow)]
pub struct OrderLine {
    pub id: Uuid,
    pub order_id: Uuid,
    pub item_id: Option<Uuid>,
    pub name: String,
    pub unit_price_cents: i64,
    pub qty: i32,
    pub note: String,
}

async fn settings(st: &AppState, shop_id: Uuid) -> AppResult<Settings> {
    Ok(sqlx::query_as("SELECT * FROM restaurant.settings WHERE shop_id = $1")
        .bind(shop_id)
        .fetch_optional(&st.db)
        .await?
        .unwrap_or_default())
}

async fn with_lines(st: &AppState, orders: Vec<Order>) -> AppResult<Vec<Value>> {
    let ids: Vec<Uuid> = orders.iter().map(|o| o.id).collect();
    let lines: Vec<OrderLine> = sqlx::query_as("SELECT * FROM restaurant.order_items WHERE order_id = ANY($1) ORDER BY name")
        .bind(&ids)
        .fetch_all(&st.db)
        .await?;
    let mut by: HashMap<Uuid, Vec<OrderLine>> = HashMap::new();
    for l in lines {
        by.entry(l.order_id).or_default().push(l);
    }
    Ok(orders
        .into_iter()
        .map(|o| {
            let items = by.remove(&o.id).unwrap_or_default();
            let mut v = serde_json::to_value(&o).unwrap_or(Value::Null);
            v["items"] = json!(items);
            v
        })
        .collect())
}

fn restaurant_only(vertical: &str) -> AppResult<()> {
    if vertical == "restaurant" {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

// ---------------------------------------------------------------------------------------------
// Public: menu, table QR, ordering, tracking
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct PlacesQ {
    pub q: Option<String>,
    pub limit: Option<i64>,
}

/// Restaurants on the platform (open ones first), with a few dishes for the card.
pub async fn places(State(st): State<AppState>, Query(q): Query<PlacesQ>) -> AppResult<Json<Vec<Value>>> {
    let rows: Vec<(sqlx::types::Json<Value>,)> = sqlx::query_as(
        "SELECT jsonb_build_object(
            'id', s.id, 'slug', s.slug, 'name', s.name, 'description', s.description, 'logo_url', s.logo_url,
            'currency', s.currency, 'address', s.address, 'verified', s.kyb_verified_at IS NOT NULL,
            'open', COALESCE(rs.accepting_orders, true), 'delivery', COALESCE(rs.delivery, false),
            'prep_minutes', COALESCE(rs.prep_minutes, 15),
            'dishes', (SELECT COUNT(*) FROM restaurant.items i WHERE i.shop_id = s.id AND i.available),
            'from_cents', (SELECT MIN(price_cents) FROM restaurant.items i WHERE i.shop_id = s.id AND i.available),
            'images', COALESCE((SELECT jsonb_agg(image_url) FROM (SELECT image_url FROM restaurant.items i
                        WHERE i.shop_id = s.id AND i.available AND i.image_url <> '' ORDER BY position LIMIT 3) x), '[]'::jsonb))
         FROM shops s LEFT JOIN restaurant.settings rs ON rs.shop_id = s.id
         WHERE s.vertical = 'restaurant' AND ($1::text IS NULL OR s.name ILIKE '%' || $1 || '%' OR s.description ILIKE '%' || $1 || '%')
         ORDER BY COALESCE(rs.accepting_orders, true) DESC, s.created_at DESC LIMIT $2",
    )
    .bind(q.q.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
    .bind(q.limit.unwrap_or(24).clamp(1, 100))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows.into_iter().map(|r| r.0 .0).collect()))
}

pub async fn public_menu(State(st): State<AppState>, Path(slug): Path<String>) -> AppResult<Json<Value>> {
    let shop = public_shop(&st, &slug).await?;
    restaurant_only(&shop.vertical)?;
    let sections: Vec<Section> = sqlx::query_as("SELECT * FROM restaurant.sections WHERE shop_id = $1 ORDER BY position, name")
        .bind(shop.id)
        .fetch_all(&st.db)
        .await?;
    let items: Vec<Item> = sqlx::query_as("SELECT * FROM restaurant.items WHERE shop_id = $1 ORDER BY position, name")
        .bind(shop.id)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(json!({
        "shop": { "id": shop.id, "slug": shop.slug, "name": shop.name, "description": shop.description, "logo_url": shop.logo_url,
                  "currency": shop.currency, "phone": shop.phone, "address": shop.address },
        "settings": settings(&st, shop.id).await?,
        "sections": sections,
        "items": items,
    })))
}

pub async fn public_table(State(st): State<AppState>, Path(tok): Path<String>) -> AppResult<Json<Value>> {
    let row: Option<(String, i32, String, String)> = sqlx::query_as(
        "SELECT t.label, t.seats, s.slug, s.name FROM restaurant.tables t JOIN shops s ON s.id = t.shop_id WHERE t.token = $1 AND t.active",
    )
    .bind(&tok)
    .fetch_optional(&st.db)
    .await?;
    let (label, seats, slug, name) = row.ok_or(AppError::NotFound)?;
    Ok(Json(json!({ "label": label, "seats": seats, "shop_slug": slug, "shop_name": name, "token": tok })))
}

#[derive(Deserialize)]
pub struct OrderLineReq {
    pub item_id: Uuid,
    pub qty: i32,
    #[serde(default)]
    pub note: String,
}

#[derive(Deserialize)]
pub struct PlaceOrder {
    pub mode: String,
    pub table_token: Option<String>,
    pub items: Vec<OrderLineReq>,
    #[serde(default)]
    pub customer_name: String,
    #[serde(default)]
    pub customer_phone: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub note: String,
}

/// Local ("020 5555 1234") or international numbers, 8–15 digits.
fn phone_ok(p: &str) -> bool {
    let d = p.chars().filter(|c| c.is_ascii_digit()).count();
    (8..=15).contains(&d) && p.chars().all(|c| c.is_ascii_digit() || " +-()".contains(c))
}

pub async fn place_order(State(st): State<AppState>, headers: HeaderMap, Path(slug): Path<String>, Json(r): Json<PlaceOrder>) -> AppResult<Json<Value>> {
    let shop = public_shop(&st, &slug).await?;
    restaurant_only(&shop.vertical)?;
    let set = settings(&st, shop.id).await?;
    if !set.accepting_orders {
        return Err(AppError::bad("this restaurant isn't taking orders right now"));
    }
    let enabled = match r.mode.as_str() {
        "dine_in" => set.dine_in,
        "takeaway" => set.takeaway,
        "delivery" => set.delivery,
        _ => false,
    };
    if !enabled {
        return Err(AppError::bad("this order type isn't available here"));
    }
    let table: Option<(Uuid, String)> = if r.mode == "dine_in" {
        let t: Option<(Uuid, String)> =
            sqlx::query_as("SELECT id, label FROM restaurant.tables WHERE token = $1 AND shop_id = $2 AND active")
                .bind(r.table_token.as_deref().unwrap_or(""))
                .bind(shop.id)
                .fetch_optional(&st.db)
                .await?;
        Some(t.ok_or_else(|| AppError::bad("scan the QR code on your table to order"))?)
    } else {
        None
    };
    let phone = r.customer_phone.trim();
    if r.mode != "dine_in" && !phone_ok(phone) {
        return Err(AppError::bad("enter a phone number so the restaurant can reach you"));
    }
    if r.mode == "delivery" && r.address.trim().is_empty() {
        return Err(AppError::bad("enter a delivery address"));
    }
    if r.items.is_empty() || r.items.len() > 50 {
        return Err(AppError::bad("add at least one dish"));
    }
    if r.items.iter().any(|l| !(1..=99).contains(&l.qty)) {
        return Err(AppError::bad("quantity must be between 1 and 99"));
    }
    let ids: Vec<Uuid> = r.items.iter().map(|l| l.item_id).collect();
    let menu: Vec<(Uuid, String, i64, bool)> =
        sqlx::query_as("SELECT id, name, price_cents, available FROM restaurant.items WHERE id = ANY($1) AND shop_id = $2")
            .bind(&ids)
            .bind(shop.id)
            .fetch_all(&st.db)
            .await?;
    let menu: HashMap<Uuid, (String, i64, bool)> = menu.into_iter().map(|(id, n, p, a)| (id, (n, p, a))).collect();
    let mut subtotal: i64 = 0;
    for l in &r.items {
        let (name, price, available) = menu.get(&l.item_id).ok_or_else(|| AppError::bad("a dish on your order is no longer on the menu"))?;
        if !available {
            return Err(AppError::bad(format!("'{name}' is sold out")));
        }
        subtotal += price * l.qty as i64;
    }
    let service = (subtotal * set.service_charge_bps as i64 + 5_000) / 10_000;
    let user = optional_user(&st, &headers);
    let day = local_today();

    let mut tx = st.db.begin().await?;
    // Ticket numbers restart daily; serialise per shop.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('restaurant:' || $1::text))").bind(shop.id).execute(&mut *tx).await?;
    let (number,): (i32,) = sqlx::query_as("SELECT COALESCE(MAX(number), 0) + 1 FROM restaurant.orders WHERE shop_id = $1 AND day = $2")
        .bind(shop.id)
        .bind(day)
        .fetch_one(&mut *tx)
        .await?;
    let order: Order = sqlx::query_as(
        "INSERT INTO restaurant.orders (shop_id, number, day, mode, table_id, table_label, user_id, customer_name, customer_phone,
            address, note, subtotal_cents, service_cents, total_cents, currency, track_token)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16) RETURNING *",
    )
    .bind(shop.id)
    .bind(number)
    .bind(day)
    .bind(&r.mode)
    .bind(table.as_ref().map(|t| t.0))
    .bind(table.as_ref().map(|t| t.1.clone()).unwrap_or_default())
    .bind(user.map(|u| u.id))
    .bind(clean_text(&r.customer_name, 80))
    .bind(clean_text(phone, 30))
    .bind(clean_text(&r.address, 300))
    .bind(clean_text(&r.note, 300))
    .bind(subtotal)
    .bind(service)
    .bind(subtotal + service)
    .bind(&shop.currency)
    .bind(token(24))
    .fetch_one(&mut *tx)
    .await?;
    for l in &r.items {
        let (name, price, _) = &menu[&l.item_id];
        sqlx::query("INSERT INTO restaurant.order_items (order_id, item_id, name, unit_price_cents, qty, note) VALUES ($1,$2,$3,$4,$5,$6)")
            .bind(order.id)
            .bind(l.item_id)
            .bind(name)
            .bind(price)
            .bind(l.qty)
            .bind(clean_text(&l.note, 120))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(with_lines(&st, vec![order]).await?.remove(0)))
}

pub async fn track(State(st): State<AppState>, Path(tok): Path<String>) -> AppResult<Json<Value>> {
    let order: Order = sqlx::query_as("SELECT * FROM restaurant.orders WHERE track_token = $1")
        .bind(&tok)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let (name, slug, phone): (String, String, String) = sqlx::query_as("SELECT name, slug, phone FROM shops WHERE id = $1")
        .bind(order.shop_id)
        .fetch_one(&st.db)
        .await?;
    let prep = settings(&st, order.shop_id).await?.prep_minutes;
    let mut v = with_lines(&st, vec![order]).await?.remove(0);
    v["shop"] = json!({ "name": name, "slug": slug, "phone": phone, "prep_minutes": prep });
    Ok(Json(v))
}

// ---------------------------------------------------------------------------------------------
// Seller: settings, menu, tables
// ---------------------------------------------------------------------------------------------

pub async fn overview(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, id, &user).await?;
    let sections: Vec<Section> = sqlx::query_as("SELECT * FROM restaurant.sections WHERE shop_id = $1 ORDER BY position, name")
        .bind(id)
        .fetch_all(&st.db)
        .await?;
    let items: Vec<Item> = sqlx::query_as("SELECT * FROM restaurant.items WHERE shop_id = $1 ORDER BY position, name").bind(id).fetch_all(&st.db).await?;
    let tables: Vec<Table> = sqlx::query_as("SELECT * FROM restaurant.tables WHERE shop_id = $1 ORDER BY label").bind(id).fetch_all(&st.db).await?;
    let (orders, open, revenue): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE status NOT IN ('completed','cancelled')),
                COALESCE(SUM(total_cents) FILTER (WHERE paid AND status <> 'cancelled'), 0)::bigint
         FROM restaurant.orders WHERE shop_id = $1 AND day = $2",
    )
    .bind(id)
    .bind(local_today())
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({
        "shop": { "id": shop.id, "slug": shop.slug, "name": shop.name, "currency": shop.currency, "vertical": shop.vertical },
        "settings": settings(&st, id).await?,
        "sections": sections, "items": items, "tables": tables,
        "today": { "orders": orders, "open": open, "revenue_cents": revenue },
    })))
}

#[derive(Deserialize)]
pub struct SettingsReq {
    pub accepting_orders: bool,
    pub dine_in: bool,
    pub takeaway: bool,
    pub delivery: bool,
    pub service_charge_bps: i32,
    pub prep_minutes: i32,
}

pub async fn save_settings(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<SettingsReq>) -> AppResult<Json<Settings>> {
    owned_shop(&st, id, &user).await?;
    if !(0..=3000).contains(&r.service_charge_bps) {
        return Err(AppError::bad("service charge must be between 0% and 30%"));
    }
    if !(0..=240).contains(&r.prep_minutes) {
        return Err(AppError::bad("preparation time must be between 0 and 240 minutes"));
    }
    if !(r.dine_in || r.takeaway || r.delivery) {
        return Err(AppError::bad("turn on at least one way to order"));
    }
    Ok(Json(
        sqlx::query_as(
            "INSERT INTO restaurant.settings (shop_id, accepting_orders, dine_in, takeaway, delivery, service_charge_bps, prep_minutes)
             VALUES ($1,$2,$3,$4,$5,$6,$7)
             ON CONFLICT (shop_id) DO UPDATE SET accepting_orders = $2, dine_in = $3, takeaway = $4, delivery = $5,
                service_charge_bps = $6, prep_minutes = $7, updated_at = now()
             RETURNING accepting_orders, dine_in, takeaway, delivery, service_charge_bps, prep_minutes",
        )
        .bind(id)
        .bind(r.accepting_orders)
        .bind(r.dine_in)
        .bind(r.takeaway)
        .bind(r.delivery)
        .bind(r.service_charge_bps)
        .bind(r.prep_minutes)
        .fetch_one(&st.db)
        .await?,
    ))
}

fn name_ok(n: &str) -> AppResult<String> {
    let n = n.trim();
    if n.is_empty() || n.chars().count() > 80 {
        return Err(AppError::bad("enter a name (up to 80 characters)"));
    }
    Ok(n.to_string())
}

#[derive(Deserialize)]
pub struct SectionReq {
    pub name: Option<String>,
    pub name_lo: Option<String>,
    pub position: Option<i32>,
}

pub async fn create_section(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<SectionReq>) -> AppResult<Json<Section>> {
    owned_shop(&st, id, &user).await?;
    let name = name_ok(r.name.as_deref().unwrap_or(""))?;
    Ok(Json(
        sqlx::query_as(
            "INSERT INTO restaurant.sections (shop_id, name, name_lo, position)
             VALUES ($1, $2, $3, COALESCE($4, (SELECT COALESCE(MAX(position), 0) + 1 FROM restaurant.sections WHERE shop_id = $1))) RETURNING *",
        )
        .bind(id)
        .bind(name)
        .bind(clean_text(r.name_lo.as_deref().unwrap_or(""), 80))
        .bind(r.position)
        .fetch_one(&st.db)
        .await?,
    ))
}

async fn owned_row(st: &AppState, table: &str, id: Uuid, user: &AuthUser) -> AppResult<Uuid> {
    let shop_id: Uuid = sqlx::query_scalar(&format!("SELECT shop_id FROM restaurant.{table} WHERE id = $1"))
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(st, shop_id, user).await?;
    Ok(shop_id)
}

pub async fn update_section(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<SectionReq>) -> AppResult<Json<Section>> {
    owned_row(&st, "sections", id, &user).await?;
    let name = r.name.as_deref().map(name_ok).transpose()?;
    Ok(Json(
        sqlx::query_as(
            "UPDATE restaurant.sections SET name = COALESCE($2, name), name_lo = COALESCE($3, name_lo), position = COALESCE($4, position)
             WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(name)
        .bind(r.name_lo.as_deref().map(|s| clean_text(s, 80)))
        .bind(r.position)
        .fetch_one(&st.db)
        .await?,
    ))
}

pub async fn delete_section(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_row(&st, "sections", id, &user).await?;
    sqlx::query("DELETE FROM restaurant.sections WHERE id = $1").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ItemReq {
    pub name: Option<String>,
    pub name_lo: Option<String>,
    pub description: Option<String>,
    pub price_cents: Option<i64>,
    /// Absent = unchanged; `null` = no section.
    #[serde(default, deserialize_with = "double_option")]
    pub section_id: Option<Option<Uuid>>,
    pub image_url: Option<String>,
    pub spicy: Option<i16>,
    pub available: Option<bool>,
    pub position: Option<i32>,
}

fn double_option<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<Uuid>>, D::Error> {
    Ok(Some(Option::deserialize(d)?))
}

async fn check_item(st: &AppState, shop_id: Uuid, r: &ItemReq) -> AppResult<()> {
    if r.price_cents.is_some_and(|p| p < 0) {
        return Err(AppError::bad("price can't be negative"));
    }
    if r.spicy.is_some_and(|s| !(0..=3).contains(&s)) {
        return Err(AppError::bad("spice level must be 0 to 3"));
    }
    if r.image_url.as_deref().is_some_and(|u| !u.is_empty() && !u.starts_with("https://") && !u.starts_with("http://")) {
        return Err(AppError::bad("image must be a web address (http/https)"));
    }
    if let Some(Some(sec)) = r.section_id {
        let ok: Option<Uuid> = sqlx::query_scalar("SELECT id FROM restaurant.sections WHERE id = $1 AND shop_id = $2")
            .bind(sec)
            .bind(shop_id)
            .fetch_optional(&st.db)
            .await?;
        if ok.is_none() {
            return Err(AppError::bad("that menu section belongs to another shop"));
        }
    }
    Ok(())
}

pub async fn create_item(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<ItemReq>) -> AppResult<Json<Item>> {
    owned_shop(&st, id, &user).await?;
    let name = name_ok(r.name.as_deref().unwrap_or(""))?;
    let price = r.price_cents.ok_or_else(|| AppError::bad("price can't be negative"))?;
    check_item(&st, id, &r).await?;
    Ok(Json(
        sqlx::query_as(
            "INSERT INTO restaurant.items (shop_id, section_id, name, name_lo, description, price_cents, image_url, spicy, available, position)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9, COALESCE($10, (SELECT COALESCE(MAX(position), 0) + 1 FROM restaurant.items WHERE shop_id = $1)))
             RETURNING *",
        )
        .bind(id)
        .bind(r.section_id.flatten())
        .bind(name)
        .bind(clean_text(r.name_lo.as_deref().unwrap_or(""), 80))
        .bind(clean_text(r.description.as_deref().unwrap_or(""), 500))
        .bind(price)
        .bind(r.image_url.as_deref().unwrap_or("").trim())
        .bind(r.spicy.unwrap_or(0))
        .bind(r.available.unwrap_or(true))
        .bind(r.position)
        .fetch_one(&st.db)
        .await?,
    ))
}

pub async fn update_item(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<ItemReq>) -> AppResult<Json<Item>> {
    let shop_id = owned_row(&st, "items", id, &user).await?;
    let name = r.name.as_deref().map(name_ok).transpose()?;
    check_item(&st, shop_id, &r).await?;
    Ok(Json(
        sqlx::query_as(
            "UPDATE restaurant.items SET name = COALESCE($2, name), name_lo = COALESCE($3, name_lo), description = COALESCE($4, description),
                price_cents = COALESCE($5, price_cents), section_id = CASE WHEN $6 THEN $7 ELSE section_id END,
                image_url = COALESCE($8, image_url), spicy = COALESCE($9, spicy), available = COALESCE($10, available),
                position = COALESCE($11, position)
             WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(name)
        .bind(r.name_lo.as_deref().map(|s| clean_text(s, 80)))
        .bind(r.description.as_deref().map(|s| clean_text(s, 500)))
        .bind(r.price_cents)
        .bind(r.section_id.is_some())
        .bind(r.section_id.flatten())
        .bind(r.image_url.as_deref().map(str::trim))
        .bind(r.spicy)
        .bind(r.available)
        .bind(r.position)
        .fetch_one(&st.db)
        .await?,
    ))
}

pub async fn delete_item(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_row(&st, "items", id, &user).await?;
    sqlx::query("DELETE FROM restaurant.items WHERE id = $1").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct TableReq {
    pub label: Option<String>,
    pub seats: Option<i32>,
    pub active: Option<bool>,
    /// true = issue a new QR token (old printed codes stop working).
    #[serde(default)]
    pub new_code: bool,
}

fn table_conflict(e: sqlx::Error) -> AppError {
    match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict("a table with that name already exists".into()),
        o => o,
    }
}

pub async fn create_table(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<TableReq>) -> AppResult<Json<Table>> {
    owned_shop(&st, id, &user).await?;
    let label = name_ok(r.label.as_deref().unwrap_or(""))?;
    let seats = r.seats.unwrap_or(4);
    if !(1..=100).contains(&seats) {
        return Err(AppError::bad("seats must be between 1 and 100"));
    }
    Ok(Json(
        sqlx::query_as("INSERT INTO restaurant.tables (shop_id, label, seats, token) VALUES ($1,$2,$3,$4) RETURNING *")
            .bind(id)
            .bind(label)
            .bind(seats)
            .bind(token(16))
            .fetch_one(&st.db)
            .await
            .map_err(table_conflict)?,
    ))
}

pub async fn update_table(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<TableReq>) -> AppResult<Json<Table>> {
    owned_row(&st, "tables", id, &user).await?;
    let label = r.label.as_deref().map(name_ok).transpose()?;
    if r.seats.is_some_and(|s| !(1..=100).contains(&s)) {
        return Err(AppError::bad("seats must be between 1 and 100"));
    }
    Ok(Json(
        sqlx::query_as(
            "UPDATE restaurant.tables SET label = COALESCE($2, label), seats = COALESCE($3, seats), active = COALESCE($4, active),
                token = CASE WHEN $5 THEN $6 ELSE token END WHERE id = $1 RETURNING *",
        )
        .bind(id)
        .bind(label)
        .bind(r.seats)
        .bind(r.active)
        .bind(r.new_code)
        .bind(token(16))
        .fetch_one(&st.db)
        .await
        .map_err(table_conflict)?,
    ))
}

pub async fn delete_table(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_row(&st, "tables", id, &user).await?;
    sqlx::query("DELETE FROM restaurant.tables WHERE id = $1").bind(id).execute(&st.db).await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------------------------------------------------------------------------------------------
// Seller: kitchen board
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct BoardQ {
    /// active (default) · done · all
    pub view: Option<String>,
    pub day: Option<NaiveDate>,
}

pub async fn board(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Query(q): Query<BoardQ>) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, id, &user).await?;
    let filter = match q.view.as_deref().unwrap_or("active") {
        "done" => "status IN ('completed','cancelled')",
        "all" => "TRUE",
        _ => "(status NOT IN ('completed','cancelled') OR (status = 'completed' AND NOT paid))",
    };
    let orders: Vec<Order> = sqlx::query_as(&format!(
        "SELECT * FROM restaurant.orders WHERE shop_id = $1 AND day = $2 AND {filter} ORDER BY created_at LIMIT 300"
    ))
    .bind(id)
    .bind(q.day.unwrap_or_else(local_today))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(with_lines(&st, orders).await?))
}

#[derive(Deserialize)]
pub struct OrderUpdate {
    pub status: Option<String>,
    pub paid: Option<bool>,
    pub payment_method: Option<String>,
}

/// Is `to` a valid next status from `from`?
pub fn can_move(from: &str, to: &str) -> bool {
    if from == to {
        return true;
    }
    if matches!(from, "completed" | "cancelled") {
        return false;
    }
    if to == "cancelled" {
        return true;
    }
    match (FLOW.iter().position(|s| *s == from), FLOW.iter().position(|s| *s == to)) {
        (Some(a), Some(b)) => b > a,
        _ => false,
    }
}

pub async fn update_order(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<OrderUpdate>) -> AppResult<Json<Value>> {
    let o: Order = sqlx::query_as("SELECT * FROM restaurant.orders WHERE id = $1").bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
    owned_shop(&st, o.shop_id, &user).await?;
    if let Some(s) = &r.status {
        if !can_move(&o.status, s) {
            return Err(AppError::Conflict(format!("can't move an order from {} to {s}", o.status)));
        }
    }
    if let Some(m) = &r.payment_method {
        if !PAYMENT_METHODS.contains(&m.as_str()) {
            return Err(AppError::bad("payment method must be cash, transfer or card"));
        }
    }
    if r.paid == Some(true) && (o.status == "cancelled" || r.status.as_deref() == Some("cancelled")) {
        return Err(AppError::bad("a cancelled order can't be marked paid"));
    }
    let o: Order = sqlx::query_as(
        "UPDATE restaurant.orders SET status = COALESCE($2, status), paid = COALESCE($3, paid),
            payment_method = COALESCE($4, payment_method), updated_at = now() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&r.status)
    .bind(r.paid)
    .bind(&r.payment_method)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(with_lines(&st, vec![o]).await?.remove(0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kitchen_flow() {
        assert!(can_move("new", "preparing"));
        assert!(can_move("ready", "completed"));
        assert!(!can_move("ready", "new"));
        assert!(can_move("preparing", "cancelled"));
        assert!(!can_move("completed", "cancelled"));
        assert!(!can_move("cancelled", "new"));
        assert!(!can_move("new", "lost"));
    }

    #[test]
    fn phones() {
        assert!(phone_ok("020 5555 1234"));
        assert!(phone_ok("+856 20 5555 1234"));
        assert!(!phone_ok("1234"));
        assert!(!phone_ok("call me"));
    }
}
