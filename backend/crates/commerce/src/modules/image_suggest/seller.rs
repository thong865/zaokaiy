//! Seller side: suggestions while typing a product name, picking one into the library, and the
//! "share my product photos" setting.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use super::{cfg, store, text};
use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    routes::{media::MediaAsset, owned_shop},
    AppState,
};

pub const ASSET_COLS: &str = "a.id, a.shop_id, a.kind, a.source, a.mime, a.url, a.thumb_url, a.storage_key, a.thumb_key, a.size_bytes,
    a.width, a.height, a.alt, a.original_name, a.created_at, a.variants, a.variant_keys, a.placeholder, a.dominant_color";

pub async fn synonym_groups(db: &sqlx::PgPool) -> AppResult<Vec<Vec<String>>> {
    Ok(sqlx::query_scalar("SELECT terms FROM image_suggest_synonyms").fetch_all(db).await?)
}

/// One suggested photo.
#[derive(Debug, Serialize, FromRow, Clone)]
pub struct Candidate {
    /// stock | own | shared
    pub source: String,
    pub id: Uuid,
    pub title: String,
    /// Brand / credit (stock) or the shop's name (shared).
    pub subtitle: String,
    #[serde(skip)]
    pub text: String,
    pub url: String,
    pub thumb_url: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub variants: Value,
    pub placeholder: String,
    pub dominant_color: String,
    #[serde(skip)]
    pub popularity: i32,
    #[serde(skip)]
    pub category_id: Option<Uuid>,
    #[sqlx(default)]
    pub score: f64,
}

#[derive(Deserialize)]
pub struct SuggestQ {
    pub q: String,
    pub limit: Option<usize>,
    /// The marketplace category chosen in the form (stock photos in it rank a little higher).
    pub category_id: Option<Uuid>,
}

pub async fn suggest(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Query(q): Query<SuggestQ>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let raw: String = q.q.chars().take(120).collect();
    let query = text::expand(&raw, &synonym_groups(&st.db).await?);
    let limit = q.limit.unwrap_or(12).clamp(1, 40);
    if query.compact.chars().count() < 2 {
        return Ok(Json(json!({ "items": [], "query": raw })));
    }
    let pats = text::patterns(&query);

    let stock: Vec<Candidate> = sqlx::query_as(
        "SELECT 'stock' AS source, s.id, s.title, CASE WHEN s.brand <> '' THEN s.brand ELSE s.credit END AS subtitle, s.search_norm AS text,
                s.url, s.thumb_url, s.width, s.height, s.variants, s.placeholder, s.dominant_color, s.use_count AS popularity, s.category_id
         FROM stock_images s WHERE s.active AND s.search_norm LIKE ANY($1) LIMIT 400",
    )
    .bind(&pats)
    .fetch_all(&st.db)
    .await?;

    let own: Vec<Candidate> = sqlx::query_as(
        "SELECT 'own' AS source, a.id, COALESCE(NULLIF(a.alt, ''), NULLIF(pn.names, ''), a.original_name) AS title, '' AS subtitle,
                image_suggest_norm(a.alt || ' ' || a.original_name || ' ' || COALESCE(pn.names, '')) AS text,
                a.url, a.thumb_url, a.width, a.height, a.variants, a.placeholder, a.dominant_color, 0 AS popularity, NULL::uuid AS category_id
         FROM media_assets a
         LEFT JOIN LATERAL (SELECT string_agg(p.name, ' ') AS names FROM product_media pm JOIN products p ON p.id = pm.product_id WHERE pm.asset_id = a.id) pn ON true
         WHERE a.shop_id = $2 AND a.kind = 'image'
           AND image_suggest_norm(a.alt || ' ' || a.original_name || ' ' || COALESCE(pn.names, '')) LIKE ANY($1)
         ORDER BY a.created_at DESC LIMIT 200",
    )
    .bind(&pats)
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;

    let shared: Vec<Candidate> = if cfg().shared {
        sqlx::query_as(
            "SELECT DISTINCT ON (a.id) 'shared' AS source, a.id, p.name AS title, s.name AS subtitle,
                    image_suggest_norm(p.name || ' ' || a.alt) AS text,
                    a.url, a.thumb_url, a.width, a.height, a.variants, a.placeholder, a.dominant_color, 0 AS popularity, p.category_id
             FROM product_media pm
             JOIN media_assets a ON a.id = pm.asset_id
             JOIN products p ON p.id = pm.product_id
             JOIN shops s ON s.id = p.shop_id
             JOIN image_share_optin o ON o.shop_id = p.shop_id
             WHERE a.kind = 'image' AND p.status = 'active' AND p.review_status = 'approved' AND p.shop_id <> $2
               AND NOT EXISTS (SELECT 1 FROM image_suggest_picks k WHERE k.asset_id = a.id)
               AND image_suggest_norm(p.name || ' ' || a.alt) LIKE ANY($1)
             LIMIT 400",
        )
        .bind(&pats)
        .bind(shop_id)
        .fetch_all(&st.db)
        .await?
    } else {
        Vec::new()
    };

    let mut items: Vec<Candidate> = stock
        .into_iter()
        .chain(own)
        .chain(shared)
        .filter_map(|mut c| {
            let base = text::score(&query, &c.text, &text::norm(&c.title));
            if base < 3.0 {
                return None;
            }
            let bonus = match c.source.as_str() {
                "stock" => 2.0 + (c.popularity.max(0) as f64 + 1.0).ln() * 0.5,
                "own" => 1.5,
                _ => 0.0,
            } + if q.category_id.is_some() && c.category_id == q.category_id { 1.0 } else { 0.0 };
            c.score = ((base + bonus) * 100.0).round() / 100.0;
            Some(c)
        })
        .collect();
    items.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
    items.truncate(limit);
    Ok(Json(json!({ "items": items, "query": raw, "synonyms": query.synonyms })))
}

#[derive(Deserialize)]
pub struct PickReq {
    /// stock | own | shared
    pub source: String,
    pub id: Uuid,
    /// What the seller typed (kept for statistics).
    #[serde(default)]
    pub query: String,
}

/// Put the chosen photo in the shop's media library and return it as a normal media asset (the
/// product form then adds it to the gallery). Stock and shared photos are copied; picking the same
/// photo again returns the earlier copy.
pub async fn pick(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<PickReq>) -> AppResult<Json<MediaAsset>> {
    owned_shop(&st, shop_id, &user).await?;
    let one_sql = format!("SELECT {ASSET_COLS} FROM media_assets a WHERE a.id = $1");
    let one = |id: Uuid| sqlx::query_as::<_, MediaAsset>(&one_sql).bind(id);
    if r.source == "own" {
        let a = one(r.id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
        if a.shop_id != shop_id {
            return Err(AppError::Forbidden);
        }
        return Ok(Json(a));
    }
    if !matches!(r.source.as_str(), "stock" | "shared") {
        return Err(AppError::bad("source must be stock, own or shared"));
    }
    let earlier: Option<Uuid> = sqlx::query_scalar(
        "SELECT asset_id FROM image_suggest_picks WHERE shop_id = $1 AND source = $2 AND source_id = $3 AND asset_id IS NOT NULL ORDER BY id DESC LIMIT 1",
    )
    .bind(shop_id)
    .bind(&r.source)
    .bind(r.id)
    .fetch_optional(&st.db)
    .await?;
    if let Some(aid) = earlier {
        if let Some(a) = one(aid).fetch_optional(&st.db).await? {
            return Ok(Json(a));
        }
    }

    #[derive(FromRow)]
    struct Src {
        title: String,
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
    let src: Src = match r.source.as_str() {
        "stock" => sqlx::query_as(
            "SELECT title, mime, url, thumb_url, storage_key, thumb_key, size_bytes, width, height, variants, variant_keys, placeholder, dominant_color
             FROM stock_images WHERE id = $1 AND active",
        )
        .bind(r.id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?,
        _ => {
            if !cfg().shared {
                return Err(AppError::NotFound);
            }
            sqlx::query_as(
                "SELECT p.name AS title, a.mime, a.url, a.thumb_url, a.storage_key, a.thumb_key, a.size_bytes, a.width, a.height,
                        a.variants, a.variant_keys, a.placeholder, a.dominant_color
                 FROM media_assets a JOIN product_media pm ON pm.asset_id = a.id JOIN products p ON p.id = pm.product_id
                 JOIN image_share_optin o ON o.shop_id = p.shop_id
                 WHERE a.id = $1 AND a.kind = 'image' AND p.status = 'active' AND p.review_status = 'approved' AND p.shop_id <> $2
                 LIMIT 1",
            )
            .bind(r.id)
            .bind(shop_id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?
        }
    };
    let files = store::Files {
        mime: src.mime,
        url: src.url,
        thumb_url: src.thumb_url,
        storage_key: src.storage_key,
        thumb_key: src.thumb_key,
        size_bytes: src.size_bytes,
        width: src.width,
        height: src.height,
        variants: src.variants,
        variant_keys: src.variant_keys,
        placeholder: src.placeholder,
        dominant_color: src.dominant_color,
    };
    let id = Uuid::new_v4();
    let base = format!("shops/{shop_id}/{}/{id}", Utc::now().format("%Y/%m"));
    let copy = store::copy_files(&st, &files, &base).await?;
    let title: String = src.title.chars().take(300).collect();
    let mut tx = st.db.begin().await?;
    let res: Result<MediaAsset, sqlx::Error> = sqlx::query_as(&format!(
        "INSERT INTO media_assets AS a (id, shop_id, kind, source, mime, url, thumb_url, storage_key, thumb_key, size_bytes, width, height,
                                       alt, original_name, created_by, variants, variant_keys, placeholder, dominant_color)
         VALUES ($1,$2,'image',$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18) RETURNING {ASSET_COLS}"
    ))
    .bind(id)
    .bind(shop_id)
    .bind(if copy.storage_key.is_some() { "upload" } else { "url" })
    .bind(&copy.mime)
    .bind(&copy.url)
    .bind(&copy.thumb_url)
    .bind(&copy.storage_key)
    .bind(&copy.thumb_key)
    .bind(copy.size_bytes)
    .bind(copy.width)
    .bind(copy.height)
    .bind(&title)
    .bind(format!("{}:{}", r.source, title).chars().take(200).collect::<String>())
    .bind(user.id)
    .bind(&copy.variants)
    .bind(&copy.variant_keys)
    .bind(&copy.placeholder)
    .bind(&copy.dominant_color)
    .fetch_one(&mut *tx)
    .await;
    let asset = match res {
        Ok(a) => a,
        Err(e) => {
            if copy.storage_key != files.storage_key {
                store::delete_keys(&st, &copy.keys()).await;
            }
            return Err(e.into());
        }
    };
    sqlx::query("INSERT INTO image_suggest_picks (shop_id, source, source_id, asset_id, query, created_by) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(shop_id)
        .bind(&r.source)
        .bind(r.id)
        .bind(asset.id)
        .bind(r.query.chars().take(120).collect::<String>())
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    if r.source == "stock" {
        sqlx::query("UPDATE stock_images SET use_count = use_count + 1 WHERE id = $1").bind(r.id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(asset))
}

pub async fn get_sharing(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let on: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM image_share_optin WHERE shop_id = $1)").bind(shop_id).fetch_one(&st.db).await?;
    let picked_by_others: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM image_suggest_picks k JOIN media_assets a ON a.id = k.source_id
         WHERE k.source = 'shared' AND a.shop_id = $1",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({ "share": on, "picked_by_others": picked_by_others, "available": cfg().shared })))
}

#[derive(Deserialize)]
pub struct SharingReq {
    pub share: bool,
}

pub async fn set_sharing(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<SharingReq>) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    if r.share {
        sqlx::query("INSERT INTO image_share_optin (shop_id) VALUES ($1) ON CONFLICT DO NOTHING").bind(shop_id).execute(&st.db).await?;
    } else {
        sqlx::query("DELETE FROM image_share_optin WHERE shop_id = $1").bind(shop_id).execute(&st.db).await?;
    }
    get_sharing(State(st), user, Path(shop_id)).await
}
