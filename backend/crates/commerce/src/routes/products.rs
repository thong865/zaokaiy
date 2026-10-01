use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::Product,
    routes::{media, owned_product, owned_shop, review},
    AppState,
};

#[derive(Deserialize)]
pub struct CreateProduct {
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub price_cents: i64,
    /// External image URLs — registered in the shop media library and attached as the gallery.
    pub images: Option<Vec<String>>,
    /// Existing library assets to use as the gallery (in order).
    pub media_ids: Option<Vec<Uuid>>,
    /// Legacy: marketplace category slug (used when `category_id` is absent).
    pub category: Option<String>,
    /// Marketplace category (required before publishing).
    pub category_id: Option<Uuid>,
    /// The seller's own storefront category.
    pub shop_category_id: Option<Uuid>,
    pub status: Option<String>,
    pub barcode: Option<String>,
    /// Short code for comment ordering, e.g. "A01".
    pub social_code: Option<String>,
    pub initial_stock: Option<i32>,
    pub low_stock_threshold: Option<i32>,
    pub allow_resell: Option<bool>,
    pub commission_bps: Option<i32>,
    /// Listing extensions from other cores, by key (e.g. `vehicle` spec sheet).
    #[serde(flatten)]
    pub ext: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
pub struct UpdateProduct {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price_cents: Option<i64>,
    /// Empty string clears the barcode.
    pub barcode: Option<String>,
    /// Empty string clears the social code.
    pub social_code: Option<String>,
    pub category_id: Option<Uuid>,
    /// Absent = unchanged, `null` = remove from shop category.
    #[serde(default, deserialize_with = "double_option")]
    pub shop_category_id: Option<Option<Uuid>>,
    pub status: Option<String>,
    pub low_stock_threshold: Option<i32>,
    pub allow_resell: Option<bool>,
    pub commission_bps: Option<i32>,
    /// Listing extensions from other cores, by key (e.g. `vehicle` spec sheet — replaces the stored one).
    #[serde(flatten)]
    pub ext: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
pub struct ListQ {
    pub status: Option<String>,
    pub review_status: Option<String>,
    pub shop_category_id: Option<Uuid>,
    pub q: Option<String>,
}

fn double_option<'de, D>(d: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<Uuid>::deserialize(d)?))
}

/// Validate a marketplace category id; returns its slug (cached in `products.category`).
async fn marketplace_category(st: &AppState, id: Uuid) -> AppResult<String> {
    let row: Option<(String, bool)> =
        sqlx::query_as("SELECT slug, active FROM categories WHERE id = $1 AND shop_id IS NULL")
            .bind(id)
            .fetch_optional(&st.db)
            .await?;
    match row {
        Some((slug, true)) => Ok(slug),
        Some((_, false)) => Err(AppError::bad("that marketplace category is no longer available")),
        None => Err(AppError::bad("unknown marketplace category")),
    }
}

async fn check_shop_category(st: &AppState, shop_id: Uuid, id: Uuid) -> AppResult<()> {
    let ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM categories WHERE id = $1 AND shop_id = $2)")
        .bind(id)
        .bind(shop_id)
        .fetch_one(&st.db)
        .await?;
    if ok { Ok(()) } else { Err(AppError::bad("shop category not found in this shop")) }
}

fn check_status(s: &Option<String>) -> AppResult<()> {
    match s.as_deref() {
        None | Some("draft" | "active" | "archived") => Ok(()),
        _ => Err(AppError::bad("status must be draft, active or archived")),
    }
}

/// Social codes: 1–10 letters/digits, upper-cased (e.g. A01, SHIRT3).
fn clean_code(c: Option<&str>) -> AppResult<Option<String>> {
    match c.map(str::trim).filter(|c| !c.is_empty()) {
        None => Ok(None),
        Some(c) if c.len() <= 10 && c.chars().all(|x| x.is_ascii_alphanumeric()) && c.chars().any(|x| x.is_ascii_alphabetic()) => {
            Ok(Some(c.to_uppercase()))
        }
        Some(_) => Err(AppError::bad("social code must be 1–10 letters/digits and contain a letter, e.g. A01")),
    }
}

fn check_bps(b: Option<i32>) -> AppResult<()> {
    match b {
        Some(v) if !(0..=9000).contains(&v) => {
            Err(AppError::bad("commission_bps must be 0..9000 (0-90%)"))
        }
        _ => Ok(()),
    }
}

pub async fn list_for_shop(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<ListQ>,
) -> AppResult<Json<Vec<Product>>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows = sqlx::query_as(
        "SELECT * FROM products
         WHERE shop_id = $1
           AND ($2::text IS NULL OR status = $2)
           AND ($3::text IS NULL OR name ILIKE '%' || $3 || '%' OR sku ILIKE '%' || $3 || '%')
           AND ($4::text IS NULL OR review_status = $4)
           AND ($5::uuid IS NULL OR shop_category_id = $5)
         ORDER BY updated_at DESC",
    )
    .bind(shop_id)
    .bind(q.status.filter(|s| !s.is_empty()))
    .bind(q.q.filter(|s| !s.trim().is_empty()))
    .bind(q.review_status.filter(|s| !s.is_empty()))
    .bind(q.shop_category_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

pub async fn get_owned(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Product>> {
    Ok(Json(owned_product(&st, id, &user).await?))
}

pub async fn create(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<CreateProduct>,
) -> AppResult<Json<Product>> {
    owned_shop(&st, shop_id, &user).await?;
    check_status(&req.status)?;
    check_bps(req.commission_bps)?;
    if req.name.trim().is_empty() || req.sku.trim().is_empty() {
        return Err(AppError::bad("name and sku are required"));
    }
    if req.price_cents < 0 {
        return Err(AppError::bad("price_cents must be >= 0"));
    }
    let stock = req.initial_stock.unwrap_or(0);
    if stock < 0 {
        return Err(AppError::bad("initial_stock must be >= 0"));
    }
    let category_id = match (req.category_id, req.category.as_deref()) {
        (Some(id), _) => Some(id),
        (None, Some(slug)) => sqlx::query_scalar("SELECT id FROM categories WHERE shop_id IS NULL AND slug = $1")
            .bind(slug.trim().to_lowercase())
            .fetch_optional(&st.db)
            .await?,
        _ => None,
    };
    let category_slug = match category_id {
        Some(id) => marketplace_category(&st, id).await?,
        None => "general".into(),
    };
    if let Some(sc) = req.shop_category_id {
        check_shop_category(&st, shop_id, sc).await?;
    }
    let extensions = st.reg.listing_inputs(&req.ext)?;

    let mut tx = st.db.begin().await?;
    let p: Product = sqlx::query_as(
        "INSERT INTO products (shop_id, sku, name, description, price_cents, images, category, status,
                               stock, low_stock_threshold, allow_resell, commission_bps, category_id, shop_category_id, barcode, social_code)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16) RETURNING *",
    )
    .bind(shop_id)
    .bind(req.sku.trim())
    .bind(req.name.trim())
    .bind(req.description.unwrap_or_default())
    .bind(req.price_cents)
    .bind(serde_json::json!([]))
    .bind(category_slug)
    .bind(req.status.unwrap_or_else(|| "draft".into()))
    .bind(stock)
    .bind(req.low_stock_threshold.unwrap_or(5))
    .bind(req.allow_resell.unwrap_or(false))
    .bind(req.commission_bps.unwrap_or(1000))
    .bind(category_id)
    .bind(req.shop_category_id)
    .bind(req.barcode.as_deref().map(str::trim).filter(|b| !b.is_empty()))
    .bind(clean_code(req.social_code.as_deref())?)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict("SKU, barcode or social code already exists in this shop".into()),
        o => o,
    })?;
    if stock > 0 {
        sqlx::query(
            "INSERT INTO inventory_movements (product_id, delta, stock_after, reason, note, created_by)
             VALUES ($1,$2,$2,'restock','initial stock',$3)",
        )
        .bind(p.id)
        .bind(stock)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    }

    // Gallery: existing library assets first, then external URLs (registered as assets).
    let mut asset_ids: Vec<Uuid> = Vec::new();
    if let Some(ids) = req.media_ids.filter(|v| !v.is_empty()) {
        let owned: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM media_assets WHERE id = ANY($1) AND shop_id = $2",
        )
        .bind(&ids)
        .bind(shop_id)
        .fetch_all(&mut *tx)
        .await?;
        asset_ids.extend(ids.into_iter().filter(|id| owned.contains(id)));
    }
    for url in req.images.unwrap_or_default().into_iter().filter(|u| u.starts_with("http")) {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO media_assets (shop_id, kind, source, url, thumb_url, alt, created_by)
             VALUES ($1,'image','url',$2,$2,$3,$4) RETURNING id",
        )
        .bind(shop_id)
        .bind(&url)
        .bind(req.name.trim())
        .bind(user.id)
        .fetch_one(&mut *tx)
        .await?;
        asset_ids.push(id);
    }
    asset_ids.dedup();
    asset_ids.truncate(media::MAX_GALLERY);
    if !asset_ids.is_empty() {
        sqlx::query(
            "INSERT INTO product_media (product_id, asset_id, position)
             SELECT $1, id, (ord - 1)::int FROM unnest($2::uuid[]) WITH ORDINALITY AS t(id, ord)",
        )
        .bind(p.id)
        .bind(&asset_ids)
        .execute(&mut *tx)
        .await?;
        media::sync_product_images(&mut *tx, p.id).await?;
    }
    for (ext, input) in &extensions {
        ext.save(&mut tx, p.id, input).await?;
    }
    // Publishing on create submits for review (or auto-approves).
    let p = review::apply(&mut tx, &st, p.id, user.id, false).await?;
    tx.commit().await?;
    Ok(Json(p))
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateProduct>,
) -> AppResult<Json<Product>> {
    let before = owned_product(&st, id, &user).await?;
    check_status(&req.status)?;
    check_bps(req.commission_bps)?;
    if matches!(req.price_cents, Some(p) if p < 0) {
        return Err(AppError::bad("price_cents must be >= 0"));
    }
    let category_slug = match req.category_id {
        Some(cid) => Some(marketplace_category(&st, cid).await?),
        None => None,
    };
    if let Some(Some(sc)) = req.shop_category_id {
        check_shop_category(&st, before.shop_id, sc).await?;
    }
    let content_changed = req.name.as_deref().is_some_and(|n| n.trim() != before.name)
        || req.description.as_deref().is_some_and(|d| d != before.description)
        || req.category_id.is_some_and(|c| Some(c) != before.category_id);
    let extensions = st.reg.listing_inputs(&req.ext)?;

    let mut tx = st.db.begin().await?;
    sqlx::query(
        "UPDATE products SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            price_cents = COALESCE($4, price_cents),
            category_id = COALESCE($5, category_id),
            category = COALESCE($6, category),
            shop_category_id = CASE WHEN $7 THEN $8 ELSE shop_category_id END,
            status = COALESCE($9, status),
            low_stock_threshold = COALESCE($10, low_stock_threshold),
            allow_resell = COALESCE($11, allow_resell),
            commission_bps = COALESCE($12, commission_bps),
            barcode = CASE WHEN $13::text IS NULL THEN barcode WHEN $13 = '' THEN NULL ELSE $13 END,
            social_code = CASE WHEN $14::text IS NULL THEN social_code WHEN $14 = '' THEN NULL ELSE $14 END,
            updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(req.name.map(|n| n.trim().to_string()))
    .bind(req.description)
    .bind(req.price_cents)
    .bind(req.category_id)
    .bind(category_slug)
    .bind(req.shop_category_id.is_some())
    .bind(req.shop_category_id.flatten())
    .bind(req.status)
    .bind(req.low_stock_threshold)
    .bind(req.allow_resell)
    .bind(req.commission_bps)
    .bind(req.barcode.map(|b| b.trim().to_string()))
    .bind(match req.social_code.as_deref() {
        Some(c) if c.trim().is_empty() => Some(String::new()),
        other => clean_code(other)?,
    })
    .execute(&mut *tx)
    .await
    .map_err(|e| match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict("barcode or social code already used by another product".into()),
        o => o,
    })?;
    for (ext, input) in &extensions {
        ext.save(&mut tx, id, input).await?;
    }
    let p = review::apply(&mut tx, &st, id, user.id, content_changed).await?;
    tx.commit().await?;
    Ok(Json(p))
}

pub async fn archive(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Product>> {
    owned_product(&st, id, &user).await?;
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE products SET status='archived', updated_at=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let p = review::apply(&mut tx, &st, id, user.id, false).await?;
    tx.commit().await?;
    Ok(Json(p))
}
