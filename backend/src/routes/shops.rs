use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::Shop,
    routes::owned_shop,
    AppState,
};

#[derive(Deserialize)]
pub struct CreateShop {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub currency: Option<String>,
    pub kind: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateShop {
    pub name: Option<String>,
    pub currency: Option<String>,
    pub description: Option<String>,
    pub logo_url: Option<String>,
    pub kind: Option<String>,
    // Business / tax details for receipts & invoices
    pub legal_name: Option<String>,
    pub tax_id: Option<String>,
    pub branch: Option<String>,
    pub address: Option<String>,
    pub phone: Option<String>,
    pub vat_bps: Option<i32>,
    pub prices_include_vat: Option<bool>,
    pub receipt_prefix: Option<String>,
    pub invoice_prefix: Option<String>,
    pub receipt_footer: Option<String>,
}

fn valid_prefix(p: &Option<String>) -> bool {
    p.as_deref()
        .is_none_or(|p| p.len() <= 8 && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

fn valid_slug(s: &str) -> bool {
    (3..=40).contains(&s.len())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn valid_kind(k: &str) -> bool {
    matches!(k, "seller" | "creator")
}

pub async fn my_shops(State(st): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<Shop>>> {
    let shops = sqlx::query_as("SELECT * FROM shops WHERE owner_id = $1 ORDER BY created_at")
        .bind(user.id)
        .fetch_all(&st.db)
        .await?;
    Ok(Json(shops))
}

pub async fn create_shop(
    State(st): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreateShop>,
) -> AppResult<Json<Shop>> {
    let slug = req.slug.trim().to_lowercase();
    if !valid_slug(&slug) {
        return Err(AppError::bad("slug must be 3-40 chars: a-z, 0-9, '-'"));
    }
    if req.name.trim().is_empty() {
        return Err(AppError::bad("name is required"));
    }
    let currency = req.currency.as_deref().map(str::trim).unwrap_or("THB").to_uppercase();
    if !valid_currency(&currency) {
        return Err(AppError::bad("currency must be THB, LAK or USD"));
    }
    let kind = req.kind.unwrap_or_else(|| "seller".into());
    if !valid_kind(&kind) {
        return Err(AppError::bad("kind must be seller or creator"));
    }
    let shop: Shop = sqlx::query_as(
        "INSERT INTO shops (owner_id, slug, name, description, logo_url, currency, kind)
         VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING *",
    )
    .bind(user.id)
    .bind(&slug)
    .bind(req.name.trim())
    .bind(req.description.unwrap_or_default())
    .bind(req.logo_url)
    .bind(currency)
    .bind(kind)
    .fetch_one(&st.db)
    .await
    .map_err(|e| match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict("shop slug already taken".into()),
        o => o,
    })?;
    Ok(Json(shop))
}

pub async fn update_shop(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateShop>,
) -> AppResult<Json<Shop>> {
    owned_shop(&st, id, &user).await?;
    if matches!(req.vat_bps, Some(v) if !(0..=3000).contains(&v)) {
        return Err(AppError::bad("vat_bps must be between 0 and 3000 (0-30%)"));
    }
    if !valid_prefix(&req.receipt_prefix) || !valid_prefix(&req.invoice_prefix) {
        return Err(AppError::bad("prefixes may use up to 8 letters, digits or '-'"));
    }
    let currency = req.currency.as_deref().map(|c| c.trim().to_uppercase());
    if matches!(&currency, Some(c) if !valid_currency(c)) {
        return Err(AppError::bad("currency must be THB, LAK or USD"));
    }
    if let Some(k) = &req.kind {
        if !valid_kind(k) {
            return Err(AppError::bad("kind must be seller or creator"));
        }
    }
    let shop: Shop = sqlx::query_as(
        "UPDATE shops SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            logo_url = COALESCE($4, logo_url),
            kind = COALESCE($5, kind),
            legal_name = COALESCE($6, legal_name),
            tax_id = COALESCE($7, tax_id),
            branch = COALESCE($8, branch),
            address = COALESCE($9, address),
            phone = COALESCE($10, phone),
            vat_bps = COALESCE($11, vat_bps),
            prices_include_vat = COALESCE($12, prices_include_vat),
            receipt_prefix = COALESCE($13, receipt_prefix),
            invoice_prefix = COALESCE($14, invoice_prefix),
            receipt_footer = COALESCE($15, receipt_footer),
            currency = COALESCE($16, currency)
         WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(req.name)
    .bind(req.description)
    .bind(req.logo_url)
    .bind(req.kind)
    .bind(req.legal_name.map(|s| s.trim().to_string()))
    .bind(req.tax_id.map(|s| s.trim().to_string()))
    .bind(req.branch.map(|s| s.trim().to_string()))
    .bind(req.address)
    .bind(req.phone.map(|s| s.trim().to_string()))
    .bind(req.vat_bps)
    .bind(req.prices_include_vat)
    .bind(req.receipt_prefix.map(|s| s.trim().to_uppercase()))
    .bind(req.invoice_prefix.map(|s| s.trim().to_uppercase()))
    .bind(req.receipt_footer)
    .bind(currency)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(shop))
}

fn valid_currency(c: &str) -> bool {
    matches!(c, "THB" | "LAK" | "USD")
}
