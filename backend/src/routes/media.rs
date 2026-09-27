//! Media library: upload, browse, edit, delete, and product galleries.

use axum::{
    extract::{Multipart, Path, Query, State},
    Json,
};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, PgExecutor};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    media,
    routes::{owned_product, owned_shop},
    AppState,
};

pub const MAX_GALLERY: usize = 15;
const MAX_FILES_PER_REQUEST: usize = 20;

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct MediaAsset {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub kind: String,
    pub source: String,
    pub mime: String,
    pub url: String,
    pub thumb_url: Option<String>,
    #[serde(skip)]
    pub storage_key: Option<String>,
    #[serde(skip)]
    pub thumb_key: Option<String>,
    pub size_bytes: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub alt: String,
    pub original_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct LibraryItem {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub asset: MediaAsset,
    pub product_count: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct GalleryItem {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub asset: MediaAsset,
    pub position: i32,
}

const ASSET_COLS: &str = "a.id, a.shop_id, a.kind, a.source, a.mime, a.url, a.thumb_url, a.storage_key,
     a.thumb_key, a.size_bytes, a.width, a.height, a.alt, a.original_name, a.created_at";

/// Refresh the denormalised `products.images` cache (image URLs in gallery order) used by
/// catalog listings.
pub async fn sync_product_images<'e>(ex: impl PgExecutor<'e>, product_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE products SET images = COALESCE((
            SELECT jsonb_agg(a.url ORDER BY pm.position)
            FROM product_media pm JOIN media_assets a ON a.id = pm.asset_id
            WHERE pm.product_id = $1 AND a.kind = 'image'), '[]'::jsonb), updated_at = now()
         WHERE id = $1",
    )
    .bind(product_id)
    .execute(ex)
    .await?;
    Ok(())
}

pub async fn gallery(st: &AppState, product_id: Uuid) -> AppResult<Vec<GalleryItem>> {
    Ok(sqlx::query_as(&format!(
        "SELECT {ASSET_COLS}, pm.position FROM product_media pm
         JOIN media_assets a ON a.id = pm.asset_id
         WHERE pm.product_id = $1 ORDER BY pm.position"
    ))
    .bind(product_id)
    .fetch_all(&st.db)
    .await?)
}

async fn load_asset(st: &AppState, id: Uuid, user: &AuthUser) -> AppResult<MediaAsset> {
    let a: MediaAsset = sqlx::query_as(&format!("SELECT {ASSET_COLS} FROM media_assets a WHERE a.id = $1"))
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(st, a.shop_id, user).await?;
    Ok(a)
}

// ---------------------------------------------------------------------------
// Library
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ListQ {
    pub kind: Option<String>,
    pub q: Option<String>,
    pub unused: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<ListQ>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let items: Vec<LibraryItem> = sqlx::query_as(&format!(
        "SELECT * FROM (
            SELECT {ASSET_COLS},
                   (SELECT COUNT(*) FROM product_media pm WHERE pm.asset_id = a.id) AS product_count
            FROM media_assets a
            WHERE a.shop_id = $1
              AND ($2::text IS NULL OR a.kind = $2)
              AND ($3::text IS NULL OR a.alt ILIKE '%'||$3||'%' OR a.original_name ILIKE '%'||$3||'%')
         ) x
         WHERE (NOT $4 OR x.product_count = 0)
         ORDER BY x.created_at DESC LIMIT $5 OFFSET $6"
    ))
    .bind(shop_id)
    .bind(q.kind.filter(|k| k == "image" || k == "video"))
    .bind(q.q.filter(|s| !s.trim().is_empty()))
    .bind(q.unused.unwrap_or(false))
    .bind(q.limit.unwrap_or(60).clamp(1, 200))
    .bind(q.offset.unwrap_or(0).max(0))
    .fetch_all(&st.db)
    .await?;

    let (count, bytes, images, videos): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(size_bytes),0)::bigint,
                COUNT(*) FILTER (WHERE kind='image'), COUNT(*) FILTER (WHERE kind='video')
         FROM media_assets WHERE shop_id = $1",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;

    Ok(Json(json!({
        "items": items,
        "stats": { "count": count, "bytes": bytes, "images": images, "videos": videos }
    })))
}

pub async fn get_one(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let asset = load_asset(&st, id, &user).await?;
    let products: Vec<(Uuid, String, i32)> = sqlx::query_as(
        "SELECT p.id, p.name, pm.position FROM product_media pm JOIN products p ON p.id = pm.product_id
         WHERE pm.asset_id = $1 ORDER BY p.name",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    let products: Vec<Value> = products
        .into_iter()
        .map(|(pid, name, pos)| json!({ "id": pid, "name": name, "is_cover": pos == 0 }))
        .collect();
    Ok(Json(json!({ "asset": asset, "products": products })))
}

#[derive(Deserialize)]
pub struct UploadQ {
    /// Append uploaded files to this product's gallery.
    pub product_id: Option<Uuid>,
}

/// `multipart/form-data` with one or more `file` parts and an optional `alt` part.
pub async fn upload(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<UploadQ>,
    mut form: Multipart,
) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    if let Some(pid) = q.product_id {
        let p = owned_product(&st, pid, &user).await?;
        if p.shop_id != shop_id {
            return Err(AppError::bad("product belongs to another shop"));
        }
    }

    let max_image = st.cfg.media_max_image_mb * 1_048_576;
    let max_video = st.cfg.media_max_video_mb * 1_048_576;
    let mut files: Vec<(String, Bytes)> = Vec::new();
    let mut alt = String::new();

    while let Some(field) = form
        .next_field()
        .await
        .map_err(|e| AppError::bad(format!("invalid upload: {e}")))?
    {
        match field.name() {
            Some("alt") => alt = field.text().await.unwrap_or_default().chars().take(300).collect(),
            Some("file") | Some("files") | Some("file[]") => {
                if files.len() >= MAX_FILES_PER_REQUEST {
                    return Err(AppError::bad(format!("at most {MAX_FILES_PER_REQUEST} files per upload")));
                }
                let name = field.file_name().unwrap_or("upload").chars().take(200).collect::<String>();
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad(format!("upload interrupted or too large: {e}")))?;
                if !data.is_empty() {
                    files.push((name, data));
                }
            }
            _ => {}
        }
    }
    if files.is_empty() {
        return Err(AppError::bad("no files received (use the 'file' field)"));
    }

    let mut created: Vec<MediaAsset> = Vec::new();
    let mut errors: Vec<Value> = Vec::new();
    for (name, data) in files {
        match store_one(&st, shop_id, user.id, &name, &alt, data, max_image, max_video).await {
            Ok(a) => created.push(a),
            Err(e) => errors.push(json!({ "file": name, "message": e.to_string() })),
        }
    }

    if let (Some(pid), false) = (q.product_id, created.is_empty()) {
        if let Err(e) = append_to_gallery(&st, pid, created.iter().map(|a| a.id).collect(), user.id).await {
            errors.push(json!({ "file": null, "message": e.to_string() }));
        }
    }

    Ok(Json(json!({ "assets": created, "errors": errors })))
}

#[allow(clippy::too_many_arguments)]
async fn store_one(
    st: &AppState,
    shop_id: Uuid,
    user_id: Uuid,
    name: &str,
    alt: &str,
    data: Bytes,
    max_image: usize,
    max_video: usize,
) -> AppResult<MediaAsset> {
    let p = tokio::task::spawn_blocking(move || media::process(data, max_image, max_video))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;

    let id = Uuid::new_v4();
    let base = format!("shops/{shop_id}/{}/{id}", Utc::now().format("%Y/%m"));
    let key = format!("{base}.{}", p.ext);
    st.storage.put(&key, p.main.clone(), &p.mime).await?;
    let mut p = p;
    if p.kind == "video" {
        if let Some(poster) = media::video_poster(&p.main, p.ext).await {
            p.thumb = Some(poster);
            p.thumb_mime = "image/jpeg";
            p.thumb_ext = "jpg";
        }
    }
    let thumb_key = match &p.thumb {
        Some(t) => {
            let k = format!("{base}_thumb.{}", p.thumb_ext);
            st.storage.put(&k, t.clone(), p.thumb_mime).await?;
            Some(k)
        }
        None => None,
    };
    let url = st.storage.url(&key);
    let thumb_url = thumb_key.as_deref().map(|k| st.storage.url(k));
    let stored_size = p.main.len() as i64 + p.thumb.as_ref().map(|t| t.len() as i64).unwrap_or(0);
    let default_alt = if alt.is_empty() {
        name.rsplit_once('.').map(|(n, _)| n).unwrap_or(name).replace(['_', '-'], " ")
    } else {
        alt.to_string()
    };

    let res = sqlx::query_as(&format!(
        "INSERT INTO media_assets AS a (id, shop_id, kind, source, mime, url, thumb_url, storage_key, thumb_key,
                                   size_bytes, width, height, alt, original_name, created_by)
         VALUES ($1,$2,$3,'upload',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14) RETURNING {ASSET_COLS}"
    ))
    .bind(id)
    .bind(shop_id)
    .bind(p.kind)
    .bind(&p.mime)
    .bind(&url)
    .bind(&thumb_url)
    .bind(&key)
    .bind(&thumb_key)
    .bind(stored_size)
    .bind(p.width)
    .bind(p.height)
    .bind(default_alt)
    .bind(name)
    .bind(user_id)
    .fetch_one(&st.db)
    .await;

    match res {
        Ok(a) => Ok(a),
        Err(e) => {
            st.storage.delete(&key).await;
            if let Some(k) = &thumb_key {
                st.storage.delete(k).await;
            }
            Err(e.into())
        }
    }
}

#[derive(Deserialize)]
pub struct FromUrlReq {
    pub url: String,
    pub kind: Option<String>,
    pub alt: Option<String>,
    pub product_id: Option<Uuid>,
}

/// Register an externally hosted image/video (e.g. a CDN or supplier URL) without copying it.
pub async fn add_url(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<FromUrlReq>,
) -> AppResult<Json<MediaAsset>> {
    owned_shop(&st, shop_id, &user).await?;
    let url = req.url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.len() > 2000 {
        return Err(AppError::bad("url must be an http(s) link"));
    }
    let lower = url.to_lowercase();
    let kind = req.kind.unwrap_or_else(|| {
        if [".mp4", ".webm", ".mov"].iter().any(|e| lower.split('?').next().unwrap_or("").ends_with(e)) {
            "video".into()
        } else {
            "image".into()
        }
    });
    if kind != "image" && kind != "video" {
        return Err(AppError::bad("kind must be image or video"));
    }
    let a: MediaAsset = sqlx::query_as(&format!(
        "INSERT INTO media_assets AS a (shop_id, kind, source, url, thumb_url, alt, created_by)
         VALUES ($1,$2,'url',$3,$4,$5,$6) RETURNING {ASSET_COLS}"
    ))
    .bind(shop_id)
    .bind(&kind)
    .bind(url)
    .bind(if kind == "image" { Some(url) } else { None })
    .bind(req.alt.unwrap_or_default())
    .bind(user.id)
    .fetch_one(&st.db)
    .await?;
    if let Some(pid) = req.product_id {
        let p = owned_product(&st, pid, &user).await?;
        if p.shop_id != shop_id {
            return Err(AppError::bad("product belongs to another shop"));
        }
        append_to_gallery(&st, pid, vec![a.id], user.id).await?;
    }
    Ok(Json(a))
}

#[derive(Deserialize)]
pub struct UpdateReq {
    pub alt: Option<String>,
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateReq>,
) -> AppResult<Json<MediaAsset>> {
    load_asset(&st, id, &user).await?;
    Ok(Json(
        sqlx::query_as(&format!(
            "UPDATE media_assets a SET alt = COALESCE($2, alt) WHERE id = $1 RETURNING {ASSET_COLS}"
        ))
        .bind(id)
        .bind(req.alt.map(|s| s.chars().take(300).collect::<String>()))
        .fetch_one(&st.db)
        .await?,
    ))
}

#[derive(Deserialize)]
pub struct DeleteQ {
    /// Also remove the asset from product galleries, content and ads that use it.
    pub force: Option<bool>,
}

#[derive(Deserialize)]
pub struct BulkDeleteReq {
    pub ids: Vec<Uuid>,
    pub force: Option<bool>,
}

pub async fn remove(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<DeleteQ>,
) -> AppResult<Json<Value>> {
    let a = load_asset(&st, id, &user).await?;
    delete_assets(&st, a.shop_id, vec![id], q.force.unwrap_or(false)).await
}

pub async fn bulk_remove(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<BulkDeleteReq>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    if req.ids.is_empty() || req.ids.len() > 200 {
        return Err(AppError::bad("select between 1 and 200 items"));
    }
    delete_assets(&st, shop_id, req.ids, req.force.unwrap_or(false)).await
}

async fn delete_assets(st: &AppState, shop_id: Uuid, ids: Vec<Uuid>, force: bool) -> AppResult<Json<Value>> {
    let assets: Vec<MediaAsset> = sqlx::query_as(&format!(
        "SELECT {ASSET_COLS} FROM media_assets a WHERE a.id = ANY($1) AND a.shop_id = $2"
    ))
    .bind(&ids)
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    if assets.is_empty() {
        return Err(AppError::NotFound);
    }
    let asset_ids: Vec<Uuid> = assets.iter().map(|a| a.id).collect();
    let urls: Vec<String> = assets
        .iter()
        .flat_map(|a| std::iter::once(a.url.clone()).chain(a.thumb_url.clone()))
        .collect();

    let (in_products, in_content, in_ads): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(DISTINCT product_id) FROM product_media WHERE asset_id = ANY($1)),
                (SELECT COUNT(*) FROM contents WHERE media_url = ANY($2)),
                (SELECT COUNT(*) FROM ad_campaigns WHERE image_url = ANY($2))",
    )
    .bind(&asset_ids)
    .bind(&urls)
    .fetch_one(&st.db)
    .await?;

    if !force && in_products + in_content + in_ads > 0 {
        return Err(AppError::Conflict(format!(
            "in use by {in_products} product(s), {in_content} content item(s) and {in_ads} ad(s) — delete with force to remove it everywhere"
        )));
    }

    let mut tx = st.db.begin().await?;
    let affected: Vec<Uuid> = sqlx::query_scalar(
        "DELETE FROM product_media WHERE asset_id = ANY($1) RETURNING product_id",
    )
    .bind(&asset_ids)
    .fetch_all(&mut *tx)
    .await?;
    sqlx::query("UPDATE contents SET media_url = NULL WHERE media_url = ANY($1)")
        .bind(&urls)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE ad_campaigns SET image_url = NULL WHERE image_url = ANY($1)")
        .bind(&urls)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM media_assets WHERE id = ANY($1)")
        .bind(&asset_ids)
        .execute(&mut *tx)
        .await?;
    let mut affected = affected;
    affected.sort();
    affected.dedup();
    for pid in &affected {
        sync_product_images(&mut *tx, *pid).await?;
    }
    tx.commit().await?;

    for a in &assets {
        for key in [&a.storage_key, &a.thumb_key].into_iter().flatten() {
            st.storage.delete(key).await;
        }
    }
    Ok(Json(json!({ "deleted": asset_ids.len(), "products_updated": affected.len() })))
}

// ---------------------------------------------------------------------------
// Product gallery
// ---------------------------------------------------------------------------

pub async fn get_gallery(State(st): State<AppState>, user: AuthUser, Path(pid): Path<Uuid>) -> AppResult<Json<Vec<GalleryItem>>> {
    owned_product(&st, pid, &user).await?;
    Ok(Json(gallery(&st, pid).await?))
}

#[derive(Deserialize)]
pub struct SetGalleryReq {
    /// Full ordered list; first = cover. Omitted assets are detached (not deleted).
    pub asset_ids: Vec<Uuid>,
}

pub async fn set_gallery(
    State(st): State<AppState>,
    user: AuthUser,
    Path(pid): Path<Uuid>,
    Json(req): Json<SetGalleryReq>,
) -> AppResult<Json<Vec<GalleryItem>>> {
    let p = owned_product(&st, pid, &user).await?;
    let mut ids: Vec<Uuid> = Vec::new();
    for id in req.asset_ids {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.len() > MAX_GALLERY {
        return Err(AppError::bad(format!("a product can have at most {MAX_GALLERY} media items")));
    }
    let owned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM media_assets WHERE id = ANY($1) AND shop_id = $2")
        .bind(&ids)
        .bind(p.shop_id)
        .fetch_one(&st.db)
        .await?;
    if owned as usize != ids.len() {
        return Err(AppError::bad("some media items are not in this shop's library"));
    }
    let mut tx = st.db.begin().await?;
    sqlx::query("DELETE FROM product_media WHERE product_id = $1")
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO product_media (product_id, asset_id, position)
         SELECT $1, id, (ord - 1)::int FROM unnest($2::uuid[]) WITH ORDINALITY AS t(id, ord)",
    )
    .bind(pid)
    .bind(&ids)
    .execute(&mut *tx)
    .await?;
    sync_product_images(&mut *tx, pid).await?;
    // Gallery changes are content changes → may send an approved product back to review.
    crate::routes::review::apply(&mut tx, &st, pid, user.id, true).await?;
    tx.commit().await?;
    Ok(Json(gallery(&st, pid).await?))
}

async fn append_to_gallery(st: &AppState, pid: Uuid, ids: Vec<Uuid>, actor: Uuid) -> AppResult<()> {
    let mut tx = st.db.begin().await?;
    let current: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM product_media WHERE product_id = $1")
        .bind(pid)
        .fetch_one(&mut *tx)
        .await?;
    if current as usize + ids.len() > MAX_GALLERY {
        return Err(AppError::bad(format!(
            "gallery is limited to {MAX_GALLERY} items — files were saved to your library"
        )));
    }
    sqlx::query(
        "INSERT INTO product_media (product_id, asset_id, position)
         SELECT $1, id, ($3 + ord - 1)::int FROM unnest($2::uuid[]) WITH ORDINALITY AS t(id, ord)
         ON CONFLICT DO NOTHING",
    )
    .bind(pid)
    .bind(&ids)
    .bind(current as i32)
    .execute(&mut *tx)
    .await?;
    sync_product_images(&mut *tx, pid).await?;
    crate::routes::review::apply(&mut tx, st, pid, actor, true).await?;
    tx.commit().await?;
    Ok(())
}

/// Public gallery for product pages (images + videos, in order).
pub async fn public_gallery(st: &AppState, product_id: Uuid) -> AppResult<Vec<Value>> {
    Ok(gallery(st, product_id)
        .await?
        .into_iter()
        .map(|g| {
            json!({
                "id": g.asset.id, "kind": g.asset.kind, "url": g.asset.url, "thumb_url": g.asset.thumb_url,
                "alt": g.asset.alt, "mime": g.asset.mime, "width": g.asset.width, "height": g.asset.height
            })
        })
        .collect())
}
