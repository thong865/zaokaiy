//! Vehicle sellers (car / motorbike dealers, private sellers, brokers).
//!
//! * A listing is an ordinary product plus a row in `vehicle_specs` (make, model, year, mileage…).
//! * `GET /vehicles` — public search with filters and facets; `?shop=<slug>` scopes it to one
//!   storefront (its own vehicles plus vehicles it resells as a broker).
//! * `POST /catalog/products/{id}/leads` — buyer enquiry, test-drive request, offer, reservation
//!   (deposit) or finance request. Sellers work them in `GET /shops/{id}/leads`.
//! * Selling state lives in `sale_status` (available → reserved → sold). A sold vehicle stays
//!   visible for 7 days with a "Sold" badge, then drops out of search.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::{
    auth::{optional_user, AuthUser},
    error::{AppError, AppResult},
    routes::{catalog::RESELL_OK, owned_product, owned_shop},
    AppState,
};

pub const TYPES: &[&str] = &["car", "motorbike", "truck", "van", "other"];
pub const CONDITIONS: &[&str] = &["new", "used", "reconditioned"];
pub const FUELS: &[&str] = &["petrol", "diesel", "hybrid", "electric", "lpg", "other"];
pub const TRANSMISSIONS: &[&str] = &["manual", "automatic", "cvt", "semi_auto"];
pub const DRIVES: &[&str] = &["", "fwd", "rwd", "4wd", "awd"];
pub const BODY_TYPES: &[&str] = &[
    "", "sedan", "hatchback", "suv", "crossover", "pickup", "mpv", "van", "coupe", "convertible", "wagon", "truck", "bus",
    "scooter", "underbone", "sport", "naked", "cruiser", "touring", "offroad", "other",
];
pub const SALE_STATUSES: &[&str] = &["available", "reserved", "sold"];
pub const LEAD_KINDS: &[&str] = &["enquiry", "test_drive", "offer", "reserve", "finance"];
pub const LEAD_STATUSES: &[&str] = &["new", "contacted", "scheduled", "won", "lost"];
/// Canonical spelling for common makes, so "toyota" and "TOYOTA" group together in filters.
const MAKES: &[&str] = &[
    "Toyota", "Honda", "Hyundai", "Kia", "Ford", "Isuzu", "Mitsubishi", "Nissan", "Mazda", "Suzuki", "Chevrolet", "Lexus",
    "Mercedes-Benz", "BMW", "Audi", "Volkswagen", "Volvo", "Subaru", "Peugeot", "BYD", "MG", "Changan", "Chery", "GWM",
    "Haval", "Geely", "Wuling", "Tesla", "Land Rover", "Porsche", "Kolao", "Yamaha", "Kawasaki", "Ducati", "Harley-Davidson",
    "Vespa", "SYM", "Kymco", "Royal Enfield", "KTM", "Triumph", "Hino", "Fuso", "Dongfeng", "Foton", "JAC", "Deco",
];
/// A reservation holds the vehicle for this long after the deposit is recorded.
const RESERVE_DAYS: i64 = 7;
/// Leads per phone number per hour (spam guard for the public form).
const LEADS_PER_HOUR: i64 = 5;

fn canon_make(m: &str) -> String {
    let m = m.split_whitespace().collect::<Vec<_>>().join(" ");
    MAKES.iter().find(|k| k.eq_ignore_ascii_case(&m)).map(|k| k.to_string()).unwrap_or(m)
}

fn one_of(v: &str, list: &[&str], msg: &'static str) -> AppResult<()> {
    if list.contains(&v) { Ok(()) } else { Err(AppError::bad(msg)) }
}

fn d_car() -> String { "car".into() }
fn d_used() -> String { "used".into() }
fn d_petrol() -> String { "petrol".into() }
fn d_auto() -> String { "automatic".into() }
fn d_true() -> bool { true }

/// Spec sheet sent by the seller (create/update product, or `PUT /products/{id}/vehicle`).
#[derive(Deserialize, Clone, Debug)]
pub struct VehicleInput {
    #[serde(default = "d_car")]
    pub vehicle_type: String,
    #[serde(default = "d_used")]
    pub condition: String,
    #[serde(default)]
    pub make: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub variant: String,
    pub year: Option<i32>,
    #[serde(default)]
    pub mileage_km: i32,
    #[serde(default = "d_petrol")]
    pub fuel: String,
    #[serde(default = "d_auto")]
    pub transmission: String,
    #[serde(default)]
    pub body_type: String,
    #[serde(default)]
    pub drive: String,
    pub engine_cc: Option<i32>,
    pub power_hp: Option<i32>,
    pub seats: Option<i32>,
    pub doors: Option<i32>,
    #[serde(default)]
    pub color: String,
    pub owners: Option<i32>,
    #[serde(default = "d_true")]
    pub registered: bool,
    #[serde(default)]
    pub plate_province: String,
    #[serde(default)]
    pub plate_no: String,
    #[serde(default)]
    pub vin: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub features: Vec<String>,
    pub warranty_months: Option<i32>,
    #[serde(default)]
    pub deposit_cents: i64,
    #[serde(default = "d_true")]
    pub negotiable: bool,
    #[serde(default)]
    pub finance_available: bool,
    #[serde(default)]
    pub buy_online: bool,
    pub sale_status: Option<String>,
}

fn in_range(v: Option<i32>, lo: i32, hi: i32) -> bool {
    v.is_none_or(|v| (lo..=hi).contains(&v))
}

impl VehicleInput {
    /// Validate and normalise (trim, canonical make, dedupe features, upper-case VIN/plate).
    pub fn clean(mut self) -> AppResult<Self> {
        self.make = canon_make(&self.make);
        self.model = self.model.trim().to_string();
        self.variant = self.variant.trim().to_string();
        if self.make.is_empty() || self.model.is_empty() {
            return Err(AppError::bad("make and model are required"));
        }
        if self.make.chars().count() > 40 || self.model.chars().count() > 60 || self.variant.chars().count() > 80 {
            return Err(AppError::bad("make, model or variant is too long"));
        }
        let max_year = Utc::now().format("%Y").to_string().parse::<i32>().unwrap_or(2100) + 1;
        match self.year {
            Some(y) if (1950..=max_year).contains(&y) => {}
            _ => return Err(AppError::bad("enter a valid model year")),
        }
        if !(0..=5_000_000).contains(&self.mileage_km) {
            return Err(AppError::bad("mileage must be between 0 and 5,000,000 km"));
        }
        one_of(&self.vehicle_type, TYPES, "vehicle type must be car, motorbike, truck, van or other")?;
        one_of(&self.condition, CONDITIONS, "condition must be new, used or reconditioned")?;
        one_of(&self.fuel, FUELS, "unknown fuel type")?;
        one_of(&self.transmission, TRANSMISSIONS, "unknown transmission")?;
        self.drive = self.drive.trim().to_lowercase();
        one_of(&self.drive, DRIVES, "unknown drive type")?;
        self.body_type = self.body_type.trim().to_lowercase();
        one_of(&self.body_type, BODY_TYPES, "unknown body type")?;
        if let Some(s) = &self.sale_status {
            one_of(s, SALE_STATUSES, "sale status must be available, reserved or sold")?;
        }
        if !(in_range(self.engine_cc, 0, 20000)
            && in_range(self.power_hp, 0, 3000)
            && in_range(self.seats, 1, 60)
            && in_range(self.doors, 0, 10)
            && in_range(self.owners, 0, 50)
            && in_range(self.warranty_months, 0, 240))
        {
            return Err(AppError::bad("a number in the vehicle specs is out of range"));
        }
        if self.deposit_cents < 0 {
            return Err(AppError::bad("deposit cannot be negative"));
        }
        self.vin = self.vin.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
        if !self.vin.is_empty() && !((6..=20).contains(&self.vin.len()) && self.vin.chars().all(|c| c.is_ascii_alphanumeric())) {
            return Err(AppError::bad("VIN / chassis number must be 6-20 letters or digits"));
        }
        self.plate_no = self.plate_no.trim().to_uppercase();
        self.color = self.color.trim().to_string();
        self.location = self.location.trim().to_string();
        self.plate_province = self.plate_province.trim().to_string();
        if [&self.plate_no, &self.color, &self.location, &self.plate_province].iter().any(|s| s.chars().count() > 80) {
            return Err(AppError::bad("a text field in the vehicle specs is too long"));
        }
        let mut feats: Vec<String> = Vec::new();
        for f in self.features.iter().map(|f| f.trim()).filter(|f| !f.is_empty()) {
            if !feats.iter().any(|x| x.eq_ignore_ascii_case(f)) {
                feats.push(f.chars().take(60).collect());
            }
        }
        if feats.len() > 40 {
            return Err(AppError::bad("at most 40 features"));
        }
        self.features = feats;
        Ok(self)
    }
}

/// Insert or replace the spec sheet of a product (inside the caller's transaction).
pub async fn upsert(conn: &mut PgConnection, product_id: Uuid, v: &VehicleInput) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO vehicle_specs (product_id, vehicle_type, condition, make, model, variant, year, mileage_km, fuel,
            transmission, body_type, drive, engine_cc, power_hp, seats, doors, color, owners, registered, plate_province,
            plate_no, vin, location, features, warranty_months, deposit_cents, negotiable, finance_available, buy_online,
            sale_status, reserved_until, sold_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,
            COALESCE($30, 'available'),
            CASE WHEN $30 = 'reserved' THEN now() + make_interval(days => $31) END,
            CASE WHEN $30 = 'sold' THEN now() END)
         ON CONFLICT (product_id) DO UPDATE SET
            vehicle_type = EXCLUDED.vehicle_type, condition = EXCLUDED.condition, make = EXCLUDED.make, model = EXCLUDED.model,
            variant = EXCLUDED.variant, year = EXCLUDED.year, mileage_km = EXCLUDED.mileage_km, fuel = EXCLUDED.fuel,
            transmission = EXCLUDED.transmission, body_type = EXCLUDED.body_type, drive = EXCLUDED.drive,
            engine_cc = EXCLUDED.engine_cc, power_hp = EXCLUDED.power_hp, seats = EXCLUDED.seats, doors = EXCLUDED.doors,
            color = EXCLUDED.color, owners = EXCLUDED.owners, registered = EXCLUDED.registered,
            plate_province = EXCLUDED.plate_province, plate_no = EXCLUDED.plate_no, vin = EXCLUDED.vin,
            location = EXCLUDED.location, features = EXCLUDED.features, warranty_months = EXCLUDED.warranty_months,
            deposit_cents = EXCLUDED.deposit_cents, negotiable = EXCLUDED.negotiable,
            finance_available = EXCLUDED.finance_available, buy_online = EXCLUDED.buy_online,
            sale_status = COALESCE($30, vehicle_specs.sale_status),
            reserved_until = CASE WHEN $30 IS NULL OR $30 = vehicle_specs.sale_status THEN vehicle_specs.reserved_until
                                  WHEN $30 = 'reserved' THEN now() + make_interval(days => $31) END,
            sold_at = CASE WHEN $30 IS NULL OR $30 = vehicle_specs.sale_status THEN vehicle_specs.sold_at
                           WHEN $30 = 'sold' THEN now() END,
            updated_at = now()",
    )
    .bind(product_id)
    .bind(&v.vehicle_type)
    .bind(&v.condition)
    .bind(&v.make)
    .bind(&v.model)
    .bind(&v.variant)
    .bind(v.year)
    .bind(v.mileage_km)
    .bind(&v.fuel)
    .bind(&v.transmission)
    .bind(&v.body_type)
    .bind(&v.drive)
    .bind(v.engine_cc)
    .bind(v.power_hp)
    .bind(v.seats)
    .bind(v.doors)
    .bind(&v.color)
    .bind(v.owners)
    .bind(v.registered)
    .bind(&v.plate_province)
    .bind(&v.plate_no)
    .bind(&v.vin)
    .bind(&v.location)
    .bind(&v.features)
    .bind(v.warranty_months)
    .bind(v.deposit_cents)
    .bind(v.negotiable)
    .bind(v.finance_available)
    .bind(v.buy_online)
    .bind(&v.sale_status)
    .bind(RESERVE_DAYS as i32)
    .execute(conn)
    .await?;
    Ok(())
}

/// Full spec sheet (seller view, includes plate number and VIN).
#[derive(Serialize, FromRow)]
pub struct VehicleSpec {
    pub product_id: Uuid,
    pub vehicle_type: String,
    pub condition: String,
    pub make: String,
    pub model: String,
    pub variant: String,
    pub year: i32,
    pub mileage_km: i32,
    pub fuel: String,
    pub transmission: String,
    pub body_type: String,
    pub drive: String,
    pub engine_cc: Option<i32>,
    pub power_hp: Option<i32>,
    pub seats: Option<i32>,
    pub doors: Option<i32>,
    pub color: String,
    pub owners: Option<i32>,
    pub registered: bool,
    pub plate_province: String,
    pub plate_no: String,
    pub vin: String,
    pub location: String,
    pub features: Vec<String>,
    pub warranty_months: Option<i32>,
    pub deposit_cents: i64,
    pub negotiable: bool,
    pub finance_available: bool,
    pub buy_online: bool,
    pub sale_status: String,
    pub reserved_until: Option<DateTime<Utc>>,
    pub sold_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

async fn load_spec(st: &AppState, product_id: Uuid) -> AppResult<Option<VehicleSpec>> {
    // Lapsed reservations fall back to available.
    sqlx::query(
        "UPDATE vehicle_specs SET sale_status = 'available', reserved_until = NULL, updated_at = now()
         WHERE product_id = $1 AND sale_status = 'reserved' AND reserved_until < now()",
    )
    .bind(product_id)
    .execute(&st.db)
    .await?;
    Ok(sqlx::query_as("SELECT * FROM vehicle_specs WHERE product_id = $1")
        .bind(product_id)
        .fetch_optional(&st.db)
        .await?)
}

/// Public view of a spec sheet: plate number hidden, VIN reduced to its last 4 characters.
pub async fn public_spec(st: &AppState, product_id: Uuid) -> AppResult<Option<Value>> {
    Ok(load_spec(st, product_id).await?.map(|s| {
        let vin_last4 = (s.vin.len() >= 4).then(|| s.vin[s.vin.len() - 4..].to_string());
        let mut v = serde_json::to_value(&s).unwrap_or(Value::Null);
        if let Some(o) = v.as_object_mut() {
            o.remove("plate_no");
            o.remove("vin");
            o.insert("vin_last4".into(), json!(vin_last4));
        }
        v
    }))
}

// ---------------------------------------------------------------------------------------------
// Seller: spec sheet
// ---------------------------------------------------------------------------------------------

pub async fn get_vehicle(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Option<VehicleSpec>>> {
    owned_product(&st, id, &user).await?;
    Ok(Json(load_spec(&st, id).await?))
}

pub async fn put_vehicle(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(v): Json<VehicleInput>,
) -> AppResult<Json<Option<VehicleSpec>>> {
    owned_product(&st, id, &user).await?;
    let v = v.clean()?;
    let mut conn = st.db.acquire().await?;
    upsert(&mut conn, id, &v).await?;
    drop(conn);
    Ok(Json(load_spec(&st, id).await?))
}

#[derive(Deserialize)]
pub struct SaleStatusReq {
    pub sale_status: String,
}

/// Mark a vehicle available / reserved / sold.
pub async fn set_sale_status(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(r): Json<SaleStatusReq>,
) -> AppResult<Json<Option<VehicleSpec>>> {
    owned_product(&st, id, &user).await?;
    one_of(&r.sale_status, SALE_STATUSES, "sale status must be available, reserved or sold")?;
    let n = sqlx::query(
        "UPDATE vehicle_specs SET
            reserved_until = CASE WHEN $2 = 'reserved' THEN COALESCE(CASE WHEN sale_status = 'reserved' THEN reserved_until END,
                                                                        now() + make_interval(days => $3)) END,
            sold_at = CASE WHEN $2 = 'sold' THEN COALESCE(CASE WHEN sale_status = 'sold' THEN sold_at END, now()) END,
            sale_status = $2, updated_at = now()
         WHERE product_id = $1",
    )
    .bind(id)
    .bind(&r.sale_status)
    .bind(RESERVE_DAYS as i32)
    .execute(&st.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::bad("this product has no vehicle details"));
    }
    Ok(Json(load_spec(&st, id).await?))
}

// ---------------------------------------------------------------------------------------------
// Public search
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize, Default)]
pub struct SearchQ {
    /// Storefront slug: that shop's vehicles plus those it resells.
    pub shop: Option<String>,
    #[serde(rename = "type")]
    pub vehicle_type: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year_min: Option<i32>,
    pub year_max: Option<i32>,
    pub price_min: Option<i64>,
    pub price_max: Option<i64>,
    pub km_max: Option<i32>,
    pub fuel: Option<String>,
    pub transmission: Option<String>,
    pub condition: Option<String>,
    pub body: Option<String>,
    pub q: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Serialize, FromRow)]
pub struct VehicleCard {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub name: String,
    pub price_cents: i64,
    pub currency: String,
    pub shop_slug: String,
    pub shop_name: String,
    pub images: Value,
    pub cover: Option<Value>,
    pub stock: i32,
    pub via_shop_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub vehicle_type: String,
    pub condition: String,
    pub make: String,
    pub model: String,
    pub variant: String,
    pub year: i32,
    pub mileage_km: i32,
    pub fuel: String,
    pub transmission: String,
    pub body_type: String,
    pub engine_cc: Option<i32>,
    pub color: String,
    pub location: String,
    pub negotiable: bool,
    pub finance_available: bool,
    pub buy_online: bool,
    pub deposit_cents: i64,
    pub sale_status: String,
    #[serde(skip)]
    pub total: i64,
}

fn nz(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// SQL for the vehicles visible in scope `$1` (NULL = whole marketplace).
fn scope_sql() -> String {
    format!(
        "SELECT p.id, p.shop_id, p.name, p.price_cents, s.currency, s.slug AS shop_slug, s.name AS shop_name, p.images, p.cover,
                p.stock, CASE WHEN $1::uuid IS NULL OR p.shop_id = $1 THEN NULL ELSE $1::uuid END AS via_shop_id, p.created_at,
                v.vehicle_type, v.condition, v.make, v.model, v.variant, v.year, v.mileage_km, v.fuel, v.transmission,
                v.body_type, v.engine_cc, v.color, v.location, v.negotiable, v.finance_available, v.buy_online,
                v.deposit_cents,
                CASE WHEN v.sale_status = 'reserved' AND v.reserved_until < now() THEN 'available' ELSE v.sale_status END AS sale_status
         FROM products p JOIN shops s ON s.id = p.shop_id JOIN vehicle_specs v ON v.product_id = p.id
         WHERE p.status = 'active' AND p.review_status = 'approved'
           AND (v.sale_status <> 'sold' OR v.sold_at > now() - interval '7 days')
           AND ($1::uuid IS NULL OR p.shop_id = $1 OR ({}))",
        RESELL_OK.replace("$via", "$1")
    )
}

pub async fn search(State(st): State<AppState>, Query(q): Query<SearchQ>) -> AppResult<Json<Value>> {
    let shop: Option<Value> = match nz(&q.shop) {
        Some(slug) => {
            let row: Option<(Uuid, String, String, String, String, String, String, Option<String>, String)> = sqlx::query_as(
                "SELECT id, slug, name, description, phone, address, currency, logo_url, vertical FROM shops WHERE slug = $1",
            )
            .bind(slug)
            .fetch_optional(&st.db)
            .await?;
            let r = row.ok_or(AppError::NotFound)?;
            Some(json!({ "id": r.0, "slug": r.1, "name": r.2, "description": r.3, "phone": r.4, "address": r.5,
                         "currency": r.6, "logo_url": r.7, "vertical": r.8 }))
        }
        None => None,
    };
    let shop_id: Option<Uuid> = shop.as_ref().and_then(|s| s["id"].as_str()).and_then(|s| s.parse().ok());
    let scope = scope_sql();
    let order = match q.sort.as_deref() {
        Some("price_asc") => "price_cents ASC, created_at DESC",
        Some("price_desc") => "price_cents DESC, created_at DESC",
        Some("year_desc") => "year DESC, created_at DESC",
        Some("km_asc") => "mileage_km ASC, created_at DESC",
        _ => "(sale_status = 'sold'), created_at DESC",
    };
    let sql = format!(
        "SELECT *, COUNT(*) OVER () AS total FROM ({scope}) x
         WHERE ($2::text IS NULL OR vehicle_type = $2)
           AND ($3::text IS NULL OR lower(make) = lower($3))
           AND ($4::text IS NULL OR lower(model) = lower($4))
           AND ($5::int IS NULL OR year >= $5) AND ($6::int IS NULL OR year <= $6)
           AND ($7::bigint IS NULL OR price_cents >= $7) AND ($8::bigint IS NULL OR price_cents <= $8)
           AND ($9::int IS NULL OR mileage_km <= $9)
           AND ($10::text IS NULL OR fuel = $10)
           AND ($11::text IS NULL OR transmission = $11)
           AND ($12::text IS NULL OR condition = $12)
           AND ($13::text IS NULL OR body_type = $13)
           AND ($14::text IS NULL OR (name || ' ' || make || ' ' || model || ' ' || variant || ' ' || location) ILIKE '%' || $14 || '%')
         ORDER BY {order} LIMIT $15 OFFSET $16"
    );
    let items: Vec<VehicleCard> = sqlx::query_as(&sql)
        .bind(shop_id)
        .bind(nz(&q.vehicle_type))
        .bind(nz(&q.make))
        .bind(nz(&q.model))
        .bind(q.year_min)
        .bind(q.year_max)
        .bind(q.price_min)
        .bind(q.price_max)
        .bind(q.km_max)
        .bind(nz(&q.fuel))
        .bind(nz(&q.transmission))
        .bind(nz(&q.condition))
        .bind(nz(&q.body))
        .bind(nz(&q.q))
        .bind(q.limit.unwrap_or(24).clamp(1, 100))
        .bind(q.offset.unwrap_or(0).max(0))
        .fetch_all(&st.db)
        .await?;
    let total = items.first().map(|i| i.total).unwrap_or(0);

    // Facets over the storefront scope (type filter applied so makes/models match the tab).
    let type_f = nz(&q.vehicle_type);
    let facet_sql = |select: &str, group: &str| {
        format!("SELECT {select} FROM ({scope}) x WHERE ($2::text IS NULL OR vehicle_type = $2) {group}")
    };
    let types: Vec<(String, i64)> = sqlx::query_as(&format!("SELECT vehicle_type, COUNT(*) FROM ({scope}) x GROUP BY 1 ORDER BY 2 DESC"))
        .bind(shop_id)
        .fetch_all(&st.db)
        .await?;
    let mm: Vec<(String, String, i64)> = sqlx::query_as(&facet_sql("make, model, COUNT(*)", "GROUP BY 1, 2 ORDER BY 1, 2"))
        .bind(shop_id)
        .bind(&type_f)
        .fetch_all(&st.db)
        .await?;
    let mut makes: Vec<Value> = Vec::new();
    for (make, model, n) in mm {
        match makes.last_mut() {
            Some(m) if m["make"] == make.as_str() => {
                m["count"] = json!(m["count"].as_i64().unwrap_or(0) + n);
                if let Some(a) = m["models"].as_array_mut() {
                    a.push(json!({ "model": model, "count": n }));
                }
            }
            _ => makes.push(json!({ "make": make, "count": n, "models": [{ "model": model, "count": n }] })),
        }
    }
    let mut counts = serde_json::Map::new();
    for col in ["fuel", "transmission", "condition", "body_type"] {
        let rows: Vec<(String, i64)> = sqlx::query_as(&facet_sql(&format!("{col}, COUNT(*)"), "GROUP BY 1 ORDER BY 2 DESC"))
            .bind(shop_id)
            .bind(&type_f)
            .fetch_all(&st.db)
            .await?;
        counts.insert(col.into(), json!(rows.into_iter().filter(|(k, _)| !k.is_empty()).collect::<Vec<_>>()));
    }
    let ranges: (Option<i32>, Option<i32>, Option<i64>, Option<i64>, Option<i32>) = sqlx::query_as(&facet_sql(
        "MIN(year), MAX(year), MIN(price_cents), MAX(price_cents), MAX(mileage_km)",
        "",
    ))
    .bind(shop_id)
    .bind(&type_f)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({
        "shop": shop,
        "items": items,
        "total": total,
        "facets": {
            "types": types,
            "makes": makes,
            "fuel": counts["fuel"], "transmission": counts["transmission"],
            "condition": counts["condition"], "body_type": counts["body_type"],
            "year": { "min": ranges.0, "max": ranges.1 },
            "price": { "min": ranges.2, "max": ranges.3 },
            "mileage": { "max": ranges.4 },
        }
    })))
}

// ---------------------------------------------------------------------------------------------
// Leads: enquiry, test drive, offer, reservation, finance
// ---------------------------------------------------------------------------------------------

/// Phone for a lead: international (+856…) or Lao local (020 xxxx xxxx, 021 xxx xxx).
pub fn lead_phone(raw: &str) -> Option<String> {
    if let Some(p) = crate::routes::oauth::normalize_phone(raw) {
        return Some(p);
    }
    let t = raw.trim();
    if !t.chars().all(|c| c.is_ascii_digit() || " -().".contains(c)) {
        return None;
    }
    let d: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
    (d.starts_with('0') && (9..=11).contains(&d.len())).then(|| format!("+856{}", &d[1..]))
}

#[derive(Deserialize)]
pub struct LeadReq {
    pub kind: String,
    pub name: String,
    pub phone: String,
    #[serde(default)]
    pub message: String,
    pub preferred_at: Option<DateTime<Utc>>,
    pub offer_cents: Option<i64>,
}

#[derive(Serialize, FromRow)]
pub struct Lead {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub product_id: Uuid,
    pub user_id: Option<Uuid>,
    pub kind: String,
    pub name: String,
    pub phone: String,
    pub message: String,
    pub preferred_at: Option<DateTime<Utc>>,
    pub offer_cents: Option<i64>,
    pub deposit_cents: i64,
    pub status: String,
    pub scheduled_at: Option<DateTime<Utc>>,
    pub deposit_paid_at: Option<DateTime<Utc>>,
    pub seller_note: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_lead(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(product_id): Path<Uuid>,
    Json(r): Json<LeadReq>,
) -> AppResult<Json<Value>> {
    let user = optional_user(&st, &headers);
    one_of(&r.kind, LEAD_KINDS, "request type must be enquiry, test_drive, offer, reserve or finance")?;
    let name = r.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::bad("enter your name"));
    }
    let phone = lead_phone(&r.phone).ok_or_else(|| AppError::bad("enter a valid phone number"))?;
    if r.message.chars().count() > 2000 {
        return Err(AppError::bad("message is too long"));
    }
    let row: Option<(Uuid, String, i64, String, String, String, String, String)> = sqlx::query_as(
        "SELECT p.shop_id, p.name, p.price_cents, s.currency, s.phone, s.payment_instructions, s.name, s.address
         FROM products p JOIN shops s ON s.id = p.shop_id
         WHERE p.id = $1 AND p.status = 'active' AND p.review_status = 'approved'",
    )
    .bind(product_id)
    .fetch_optional(&st.db)
    .await?;
    let (shop_id, _pname, price, currency, shop_phone, pay_info, shop_name, address) = row.ok_or(AppError::NotFound)?;
    let spec = load_spec(&st, product_id).await?.ok_or(AppError::NotFound)?;
    if spec.sale_status == "sold" {
        return Err(AppError::bad("this vehicle has already been sold"));
    }
    match r.kind.as_str() {
        "test_drive" => match r.preferred_at {
            None => return Err(AppError::bad("choose a date and time for the test drive")),
            Some(t) if t < Utc::now() - Duration::minutes(5) => {
                return Err(AppError::bad("the test drive time must be in the future"))
            }
            Some(t) if t > Utc::now() + Duration::days(90) => {
                return Err(AppError::bad("choose a test drive time within the next 90 days"))
            }
            _ => {}
        },
        "offer" => match r.offer_cents {
            Some(o) if o > 0 && o <= price.max(1) * 2 => {}
            Some(o) if o > 0 => return Err(AppError::bad("your offer is higher than twice the asking price")),
            _ => return Err(AppError::bad("enter the amount you want to offer")),
        },
        "reserve" if spec.sale_status == "reserved" => return Err(AppError::bad("this vehicle is already reserved")),
        _ => {}
    }

    // Same request again within 10 minutes → return the first one (double-clicks, retries).
    let dup: Option<Lead> = sqlx::query_as(
        "SELECT * FROM vehicle_leads WHERE product_id = $1 AND phone = $2 AND kind = $3 AND created_at > now() - interval '10 minutes'
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(product_id)
    .bind(&phone)
    .bind(&r.kind)
    .fetch_optional(&st.db)
    .await?;
    let (lead, duplicate) = match dup {
        Some(l) => (l, true),
        None => {
            let recent: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vehicle_leads WHERE phone = $1 AND created_at > now() - interval '1 hour'")
                .bind(&phone)
                .fetch_one(&st.db)
                .await?;
            if recent >= LEADS_PER_HOUR {
                return Err(AppError::bad("too many requests from this phone number, try again later"));
            }
            let l: Lead = sqlx::query_as(
                "INSERT INTO vehicle_leads (shop_id, product_id, user_id, kind, name, phone, message, preferred_at, offer_cents, deposit_cents)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING *",
            )
            .bind(shop_id)
            .bind(product_id)
            .bind(user.map(|u| u.id))
            .bind(&r.kind)
            .bind(name)
            .bind(&phone)
            .bind(r.message.trim())
            .bind(if r.kind == "test_drive" { r.preferred_at } else { None })
            .bind(if r.kind == "offer" { r.offer_cents } else { None })
            .bind(if r.kind == "reserve" { spec.deposit_cents } else { 0 })
            .fetch_one(&st.db)
            .await?;
            (l, false)
        }
    };
    Ok(Json(json!({
        "id": lead.id, "kind": lead.kind, "status": lead.status, "duplicate": duplicate,
        "deposit_cents": lead.deposit_cents, "currency": currency,
        "payment_instructions": if lead.kind == "reserve" { pay_info } else { String::new() },
        "contact": { "name": shop_name, "phone": shop_phone, "address": address },
    })))
}

#[derive(Deserialize)]
pub struct LeadListQ {
    pub status: Option<String>,
    pub kind: Option<String>,
    pub q: Option<String>,
}

pub async fn list_leads(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<LeadListQ>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(l) || jsonb_build_object(
                'product_name', p.name, 'price_cents', p.price_cents, 'currency', s.currency, 'cover', p.cover,
                'image', p.images->0, 'make', v.make, 'model', v.model, 'year', v.year, 'sale_status', v.sale_status,
                'vehicle_deposit_cents', v.deposit_cents)
         FROM vehicle_leads l JOIN products p ON p.id = l.product_id JOIN shops s ON s.id = l.shop_id
         LEFT JOIN vehicle_specs v ON v.product_id = l.product_id
         WHERE l.shop_id = $1 AND ($2::text IS NULL OR l.status = $2) AND ($3::text IS NULL OR l.kind = $3)
           AND ($4::text IS NULL OR (l.name || ' ' || l.phone || ' ' || p.name) ILIKE '%' || $4 || '%')
         ORDER BY (l.status = 'new') DESC, COALESCE(l.scheduled_at, l.preferred_at, l.created_at) DESC
         LIMIT 300",
    )
    .bind(shop_id)
    .bind(nz(&q.status))
    .bind(nz(&q.kind))
    .bind(nz(&q.q))
    .fetch_all(&st.db)
    .await?;
    let counts: Vec<(String, i64)> = sqlx::query_as("SELECT status, COUNT(*) FROM vehicle_leads WHERE shop_id = $1 GROUP BY 1")
        .bind(shop_id)
        .fetch_all(&st.db)
        .await?;
    let counts: serde_json::Map<String, Value> = counts.into_iter().map(|(k, n)| (k, json!(n))).collect();
    Ok(Json(json!({ "items": rows, "counts": counts })))
}

fn double_opt_dt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<DateTime<Utc>>>, D::Error> {
    Ok(Some(Option::<DateTime<Utc>>::deserialize(d)?))
}

#[derive(Deserialize)]
pub struct LeadUpdate {
    pub status: Option<String>,
    pub seller_note: Option<String>,
    /// Absent = unchanged, `null` = clear.
    #[serde(default, deserialize_with = "double_opt_dt")]
    pub scheduled_at: Option<Option<DateTime<Utc>>>,
    /// Record (true) or undo (false) the reservation deposit — reserves / frees the vehicle.
    pub deposit_paid: Option<bool>,
}

pub async fn update_lead(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(r): Json<LeadUpdate>,
) -> AppResult<Json<Lead>> {
    let lead: Lead = sqlx::query_as("SELECT * FROM vehicle_leads WHERE id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(&st, lead.shop_id, &user).await?;
    if let Some(s) = &r.status {
        one_of(s, LEAD_STATUSES, "status must be new, contacted, scheduled, won or lost")?;
    }
    if r.seller_note.as_deref().is_some_and(|n| n.chars().count() > 2000) {
        return Err(AppError::bad("note is too long"));
    }
    let mut tx = st.db.begin().await?;
    if let Some(paid) = r.deposit_paid {
        let sale: Option<String> = sqlx::query_scalar("SELECT sale_status FROM vehicle_specs WHERE product_id = $1 FOR UPDATE")
            .bind(lead.product_id)
            .fetch_optional(&mut *tx)
            .await?;
        if paid && lead.deposit_paid_at.is_none() {
            if sale.as_deref() == Some("sold") {
                return Err(AppError::bad("this vehicle has already been sold"));
            }
            let other: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM vehicle_leads WHERE product_id = $1 AND id <> $2 AND deposit_paid_at IS NOT NULL AND status <> 'lost')",
            )
            .bind(lead.product_id)
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
            if other && sale.as_deref() == Some("reserved") {
                return Err(AppError::bad("this vehicle is already reserved"));
            }
            sqlx::query(
                "UPDATE vehicle_specs SET sale_status = 'reserved', reserved_until = now() + make_interval(days => $2), updated_at = now()
                 WHERE product_id = $1",
            )
            .bind(lead.product_id)
            .bind(RESERVE_DAYS as i32)
            .execute(&mut *tx)
            .await?;
        } else if !paid && lead.deposit_paid_at.is_some() && sale.as_deref() == Some("reserved") {
            sqlx::query("UPDATE vehicle_specs SET sale_status = 'available', reserved_until = NULL, updated_at = now() WHERE product_id = $1")
                .bind(lead.product_id)
                .execute(&mut *tx)
                .await?;
        }
    }
    let lead: Lead = sqlx::query_as(
        "UPDATE vehicle_leads SET
            status = COALESCE($2, status),
            seller_note = COALESCE($3, seller_note),
            scheduled_at = CASE WHEN $4 THEN $5 ELSE scheduled_at END,
            deposit_paid_at = CASE WHEN $6::bool IS NULL THEN deposit_paid_at WHEN $6 THEN COALESCE(deposit_paid_at, now()) END,
            updated_at = now()
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(&r.status)
    .bind(r.seller_note.as_deref().map(str::trim))
    .bind(r.scheduled_at.is_some())
    .bind(r.scheduled_at.flatten())
    .bind(r.deposit_paid)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(lead))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> VehicleInput {
        serde_json::from_value(json!({ "make": " toyota ", "model": "Hilux Revo", "year": 2020 })).unwrap()
    }

    #[test]
    fn defaults_and_normalisation() {
        let v = input().clean().unwrap();
        assert_eq!(v.make, "Toyota");
        assert_eq!((v.vehicle_type.as_str(), v.condition.as_str(), v.fuel.as_str()), ("car", "used", "petrol"));
        assert!(v.negotiable && v.registered && !v.buy_online);
        let mut v = input();
        v.make = "mercedes-benz".into();
        v.vin = "mr0 fb8cd 3k1234567".into();
        v.features = vec!["ABS".into(), "abs".into(), " ".into(), "Camera".into()];
        let v = v.clean().unwrap();
        assert_eq!(v.make, "Mercedes-Benz");
        assert_eq!(v.vin, "MR0FB8CD3K1234567");
        assert_eq!(v.features, vec!["ABS", "Camera"]);
        let mut v = input();
        v.make = "Some  New Brand".into();
        assert_eq!(v.clean().unwrap().make, "Some New Brand");
    }

    #[test]
    fn rejects_bad_specs() {
        let bad = |f: fn(&mut VehicleInput)| {
            let mut v = input();
            f(&mut v);
            v.clean().is_err()
        };
        assert!(bad(|v| v.model.clear()));
        assert!(bad(|v| v.year = None));
        assert!(bad(|v| v.year = Some(1900)));
        assert!(bad(|v| v.year = Some(2200)));
        assert!(bad(|v| v.mileage_km = -1));
        assert!(bad(|v| v.fuel = "steam".into()));
        assert!(bad(|v| v.vehicle_type = "boat".into()));
        assert!(bad(|v| v.body_type = "spaceship".into()));
        assert!(bad(|v| v.seats = Some(0)));
        assert!(bad(|v| v.vin = "AB-12".into()));
        assert!(bad(|v| v.sale_status = Some("gone".into())));
        assert!(bad(|v| v.deposit_cents = -5));
        assert!(!bad(|v| v.body_type = "Pickup".into()));
    }

    #[test]
    fn phones() {
        assert_eq!(lead_phone("020 5555 1234").as_deref(), Some("+8562055551234"));
        assert_eq!(lead_phone("021-312-345").as_deref(), Some("+85621312345"));
        assert_eq!(lead_phone("+856 20 5555 1234").as_deref(), Some("+8562055551234"));
        assert_eq!(lead_phone("+66 81 234 5678").as_deref(), Some("+66812345678"));
        assert!(lead_phone("12345").is_none());
        assert!(lead_phone("call me").is_none());
        assert!(lead_phone("5555 1234").is_none());
    }
}
