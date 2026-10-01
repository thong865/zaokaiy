//! Admin side: the stock photo library (upload, promote a shop photo, edit, delete) and synonyms.

use axum::{
    extract::{Multipart, Path, Query, State},
    Json,
};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use super::{store, text};
use crate::{
    auth::AdminUser,
    error::{AppError, AppResult},
    AppState,
};

#[derive(Debug, Serialize, FromRow)]
pub struct StockImage {
    pub id: Uuid,
    pub title: String,
    pub keywords: String,
    pub brand: String,
    pub credit: String,
    pub category_id: Option<Uuid>,
    pub url: String,
    pub thumb_url: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub variants: Value,
    pub placeholder: String,
    pub dominant_color: String,
    pub active: bool,
    pub use_count: i32,
    pub source_asset_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const STOCK_COLS: &str = "id, title, keywords, brand, credit, category_id, url, thumb_url, width, height, variants, placeholder,
    dominant_color, active, use_count, source_asset_id, created_at, updated_at";

#[derive(Deserialize)]
pub struct ListQ {
    pub q: Option<String>,
    pub active: Option<bool>,
}

pub async fn list_stock(State(st): State<AppState>, _a: AdminUser, Query(q): Query<ListQ>) -> AppResult<Json<Value>> {
    let needle = q.q.as_deref().map(text::norm).filter(|s| !s.is_empty());
    let items: Vec<StockImage> = sqlx::query_as(&format!(
        "SELECT {STOCK_COLS} FROM stock_images
         WHERE ($1::text IS NULL OR strpos(search_norm, $1) > 0) AND ($2::bool IS NULL OR active = $2)
         ORDER BY created_at DESC LIMIT 300"
    ))
    .bind(needle)
    .bind(q.active)
    .fetch_all(&st.db)
    .await?;
    let (total, picks): (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM stock_images), (SELECT count(*) FROM image_suggest_picks)")
        .fetch_one(&st.db)
        .await?;
    let sharing: i64 = sqlx::query_scalar("SELECT count(*) FROM image_share_optin").fetch_one(&st.db).await?;
    Ok(Json(json!({ "items": items, "total": total, "picks": picks, "sharing_shops": sharing })))
}

#[derive(Debug, Default, Deserialize)]
pub struct Meta {
    pub title: Option<String>,
    pub keywords: Option<String>,
    pub brand: Option<String>,
    pub credit: Option<String>,
    pub category_id: Option<Uuid>,
    pub active: Option<bool>,
}

fn clean(s: Option<String>, max: usize) -> Option<String> {
    s.map(|v| v.trim().chars().take(max).collect())
}

fn check_title(t: &Option<String>) -> AppResult<()> {
    match t.as_deref() {
        Some(t) if t.chars().count() >= 2 => Ok(()),
        _ => Err(AppError::bad("give the photo a title (at least 2 characters)")),
    }
}

async fn insert_stock(st: &AppState, id: Uuid, m: Meta, f: &store::Files, source_asset: Option<Uuid>, by: Uuid) -> AppResult<StockImage> {
    let res = sqlx::query_as(&format!(
        "INSERT INTO stock_images (id, title, keywords, brand, credit, category_id, search_norm, mime, url, thumb_url, storage_key, thumb_key,
                                   size_bytes, width, height, variants, variant_keys, placeholder, dominant_color, source_asset_id, created_by)
         VALUES ($1,$2,$3,$4,$5,$6, image_suggest_norm($2 || ' ' || $3 || ' ' || $4), $7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20)
         RETURNING {STOCK_COLS}"
    ))
    .bind(id)
    .bind(clean(m.title, 200).unwrap_or_default())
    .bind(clean(m.keywords, 1000).unwrap_or_default())
    .bind(clean(m.brand, 100).unwrap_or_default())
    .bind(clean(m.credit, 300).unwrap_or_default())
    .bind(m.category_id)
    .bind(&f.mime)
    .bind(&f.url)
    .bind(&f.thumb_url)
    .bind(&f.storage_key)
    .bind(&f.thumb_key)
    .bind(f.size_bytes)
    .bind(f.width)
    .bind(f.height)
    .bind(&f.variants)
    .bind(&f.variant_keys)
    .bind(&f.placeholder)
    .bind(&f.dominant_color)
    .bind(source_asset)
    .bind(by)
    .fetch_one(&st.db)
    .await;
    match res {
        Ok(s) => Ok(s),
        Err(e) => {
            store::delete_keys(st, &f.keys()).await;
            Err(e.into())
        }
    }
}

/// Upload a stock photo: multipart `file` + `title`, `keywords`, `brand`, `credit`, `category_id`.
pub async fn upload_stock(State(st): State<AppState>, AdminUser(admin): AdminUser, mut form: Multipart) -> AppResult<Json<StockImage>> {
    let mut m = Meta::default();
    let mut file: Option<Bytes> = None;
    while let Some(field) = form.next_field().await.map_err(|e| AppError::bad(format!("invalid upload: {e}")))? {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            file = Some(field.bytes().await.map_err(|e| AppError::bad(format!("upload interrupted or too large: {e}")))?);
            continue;
        }
        let v = field.text().await.unwrap_or_default();
        match name.as_str() {
            "title" => m.title = Some(v),
            "keywords" => m.keywords = Some(v),
            "brand" => m.brand = Some(v),
            "credit" => m.credit = Some(v),
            "category_id" => m.category_id = v.trim().parse().ok(),
            _ => {}
        }
    }
    m.title = clean(m.title, 200);
    check_title(&m.title)?;
    let data = file.filter(|d| !d.is_empty()).ok_or_else(|| AppError::bad("no files received (use the 'file' field)"))?;
    let id = Uuid::new_v4();
    let files = store::store_upload(&st, &format!("stock/{}/{id}", Utc::now().format("%Y/%m")), data).await?;
    Ok(Json(insert_stock(&st, id, m, &files, None, admin.id).await?))
}

#[derive(Deserialize)]
pub struct PromoteReq {
    pub asset_id: Uuid,
    #[serde(flatten)]
    pub meta: Meta,
}

/// Copy a photo from any shop's library into the stock library (e.g. a brand's official shot,
/// used with the owner's permission).
pub async fn promote(State(st): State<AppState>, AdminUser(admin): AdminUser, Json(r): Json<PromoteReq>) -> AppResult<Json<StockImage>> {
    let mut m = r.meta;
    m.title = clean(m.title, 200);
    check_title(&m.title)?;
    #[derive(FromRow)]
    struct A {
        kind: String,
        mime: String,
        url: String,
        thumb_url: Option<String>,
        storage_key: Option<String>,
        thumb_key: Option<String>,
        size_bytes: i64,
        width: Option<i32>,
        height: Option<i32>,
        variants: Value,
        variant_keys: Vec<String>,
        placeholder: String,
        dominant_color: String,
    }
    let a: A = sqlx::query_as(
        "SELECT kind, mime, url, thumb_url, storage_key, thumb_key, size_bytes, width, height, variants, variant_keys, placeholder, dominant_color
         FROM media_assets WHERE id = $1",
    )
    .bind(r.asset_id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if a.kind != "image" {
        return Err(AppError::bad("stock photos must be images (JPG, PNG, WebP)"));
    }
    let src = store::Files {
        mime: a.mime,
        url: a.url,
        thumb_url: a.thumb_url,
        storage_key: a.storage_key,
        thumb_key: a.thumb_key,
        size_bytes: a.size_bytes,
        width: a.width,
        height: a.height,
        variants: a.variants,
        variant_keys: a.variant_keys,
        placeholder: a.placeholder,
        dominant_color: a.dominant_color,
    };
    let id = Uuid::new_v4();
    let files = store::copy_files(&st, &src, &format!("stock/{}/{id}", Utc::now().format("%Y/%m"))).await?;
    // A by-reference copy (external URL) owns no files, so a failed insert must not delete the source's.
    let owned = if files.storage_key.is_some() { files.clone() } else { store::Files { variant_keys: vec![], ..files.clone() } };
    Ok(Json(insert_stock(&st, id, m, &owned, Some(r.asset_id), admin.id).await?))
}

pub async fn update_stock(State(st): State<AppState>, _a: AdminUser, Path(id): Path<Uuid>, Json(m): Json<Meta>) -> AppResult<Json<StockImage>> {
    if m.title.is_some() {
        check_title(&clean(m.title.clone(), 200))?;
    }
    let s: Option<StockImage> = sqlx::query_as(&format!(
        "UPDATE stock_images SET title = COALESCE($2, title), keywords = COALESCE($3, keywords), brand = COALESCE($4, brand),
                credit = COALESCE($5, credit), category_id = CASE WHEN $6::bool THEN $7 ELSE category_id END, active = COALESCE($8, active),
                updated_at = now()
         WHERE id = $1 RETURNING {STOCK_COLS}"
    ))
    .bind(id)
    .bind(clean(m.title, 200))
    .bind(clean(m.keywords, 1000))
    .bind(clean(m.brand, 100))
    .bind(clean(m.credit, 300))
    .bind(m.category_id.is_some())
    .bind(m.category_id)
    .bind(m.active)
    .fetch_optional(&st.db)
    .await?;
    sqlx::query("UPDATE stock_images SET search_norm = image_suggest_norm(title || ' ' || keywords || ' ' || brand) WHERE id = $1")
        .bind(id)
        .execute(&st.db)
        .await?;
    s.map(Json).ok_or(AppError::NotFound)
}

/// Delete a stock photo and its files. Copies already picked into shop libraries stay.
pub async fn delete_stock(State(st): State<AppState>, _a: AdminUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let keys: Option<(Option<String>, Option<String>, Vec<String>)> =
        sqlx::query_as("DELETE FROM stock_images WHERE id = $1 RETURNING storage_key, thumb_key, variant_keys")
            .bind(id)
            .fetch_optional(&st.db)
            .await?;
    let (k, t, v) = keys.ok_or(AppError::NotFound)?;
    let all: Vec<String> = k.into_iter().chain(t).chain(v).collect();
    store::delete_keys(&st, &all).await;
    Ok(Json(json!({ "deleted": id })))
}

/// Shop product photos matching a name that aren't stock yet — for promoting them.
pub async fn candidates(State(st): State<AppState>, _a: AdminUser, Query(q): Query<ListQ>) -> AppResult<Json<Value>> {
    let Some(needle) = q.q.as_deref().map(text::norm).filter(|s| s.chars().count() >= 2) else {
        return Ok(Json(json!({ "items": [] })));
    };
    let rows: Vec<(sqlx::types::Json<Value>,)> = sqlx::query_as(
        "SELECT DISTINCT ON (a.id) jsonb_build_object('asset_id', a.id, 'url', a.url, 'thumb_url', a.thumb_url, 'variants', a.variants,
                 'placeholder', a.placeholder, 'width', a.width, 'height', a.height, 'product', p.name, 'shop', s.name, 'alt', a.alt)
         FROM product_media pm JOIN media_assets a ON a.id = pm.asset_id JOIN products p ON p.id = pm.product_id JOIN shops s ON s.id = p.shop_id
         WHERE a.kind = 'image' AND strpos(image_suggest_norm(p.name || ' ' || a.alt), $1) > 0
           AND NOT EXISTS (SELECT 1 FROM stock_images x WHERE x.source_asset_id = a.id)
           AND NOT EXISTS (SELECT 1 FROM image_suggest_picks k WHERE k.asset_id = a.id)
         LIMIT 60",
    )
    .bind(needle)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(json!({ "items": rows.into_iter().map(|r| r.0 .0).collect::<Vec<_>>() })))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Synonym {
    pub id: i64,
    pub terms: Vec<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn synonyms(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Vec<Synonym>>> {
    Ok(Json(sqlx::query_as("SELECT id, terms, created_at FROM image_suggest_synonyms ORDER BY id").fetch_all(&st.db).await?))
}

#[derive(Deserialize)]
pub struct SynonymReq {
    pub terms: Vec<String>,
}

pub async fn add_synonym(State(st): State<AppState>, _a: AdminUser, Json(r): Json<SynonymReq>) -> AppResult<Json<Synonym>> {
    let mut terms: Vec<String> = Vec::new();
    for t in r.terms.iter().map(|t| text::norm(t)).filter(|t| t.chars().count() >= 2) {
        if !terms.contains(&t) {
            terms.push(t.chars().take(60).collect());
        }
    }
    if !(2..=20).contains(&terms.len()) {
        return Err(AppError::bad("enter 2 to 20 words that mean the same thing"));
    }
    Ok(Json(
        sqlx::query_as("INSERT INTO image_suggest_synonyms (terms) VALUES ($1) RETURNING id, terms, created_at")
            .bind(&terms)
            .fetch_one(&st.db)
            .await?,
    ))
}

pub async fn delete_synonym(State(st): State<AppState>, _a: AdminUser, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let n = sqlx::query("DELETE FROM image_suggest_synonyms WHERE id = $1").bind(id).execute(&st.db).await?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "deleted": id })))
}
