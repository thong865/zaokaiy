use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::AdCampaign,
    routes::owned_shop,
    AppState,
};

#[derive(Deserialize)]
pub struct CreateAd {
    pub product_id: Uuid,
    pub headline: String,
    pub body: Option<String>,
    pub image_url: Option<String>,
    pub budget_cents: i64,
    pub cpc_cents: Option<i64>,
    pub status: Option<String>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct UpdateAd {
    pub headline: Option<String>,
    pub body: Option<String>,
    pub image_url: Option<String>,
    pub budget_cents: Option<i64>,
    pub cpc_cents: Option<i64>,
    pub status: Option<String>,
}

fn check_status(s: &Option<String>) -> AppResult<()> {
    match s.as_deref() {
        None | Some("draft" | "active" | "paused" | "ended") => Ok(()),
        _ => Err(AppError::bad(
            "status must be draft, active, paused or ended",
        )),
    }
}

/// A shop may advertise its own products, or products it resells via an active listing.
async fn can_advertise(st: &AppState, shop_id: Uuid, product_id: Uuid) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM products WHERE id=$2 AND shop_id=$1)
             OR EXISTS (SELECT 1 FROM listings WHERE product_id=$2 AND reseller_shop_id=$1 AND active)",
    )
    .bind(shop_id)
    .bind(product_id)
    .fetch_one(&st.db)
    .await?)
}

pub async fn list(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
) -> AppResult<Json<Vec<AdCampaign>>> {
    owned_shop(&st, shop_id, &user).await?;
    Ok(Json(
        sqlx::query_as("SELECT * FROM ad_campaigns WHERE shop_id=$1 ORDER BY created_at DESC")
            .bind(shop_id)
            .fetch_all(&st.db)
            .await?,
    ))
}

pub async fn create(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<CreateAd>,
) -> AppResult<Json<AdCampaign>> {
    owned_shop(&st, shop_id, &user).await?;
    check_status(&req.status)?;
    if !can_advertise(&st, shop_id, req.product_id).await? {
        return Err(AppError::bad(
            "you can only advertise your own or resold products",
        ));
    }
    if req.headline.trim().is_empty() || req.budget_cents <= 0 {
        return Err(AppError::bad("headline and a positive budget are required"));
    }
    let ad = sqlx::query_as(
        "INSERT INTO ad_campaigns (shop_id, product_id, headline, body, image_url, budget_cents, cpc_cents, status, starts_at, ends_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING *",
    )
    .bind(shop_id)
    .bind(req.product_id)
    .bind(req.headline.trim())
    .bind(req.body.unwrap_or_default())
    .bind(req.image_url)
    .bind(req.budget_cents)
    .bind(req.cpc_cents.unwrap_or(100).max(1))
    .bind(req.status.unwrap_or_else(|| "draft".into()))
    .bind(req.starts_at)
    .bind(req.ends_at)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(ad))
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateAd>,
) -> AppResult<Json<AdCampaign>> {
    let shop_id: Uuid = sqlx::query_scalar("SELECT shop_id FROM ad_campaigns WHERE id=$1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(&st, shop_id, &user).await?;
    check_status(&req.status)?;
    let ad = sqlx::query_as(
        "UPDATE ad_campaigns SET
            headline = COALESCE($2, headline), body = COALESCE($3, body), image_url = COALESCE($4, image_url),
            budget_cents = COALESCE($5, budget_cents), cpc_cents = COALESCE($6, cpc_cents), status = COALESCE($7, status)
         WHERE id=$1 RETURNING *",
    )
    .bind(id)
    .bind(req.headline)
    .bind(req.body)
    .bind(req.image_url)
    .bind(req.budget_cents)
    .bind(req.cpc_cents)
    .bind(req.status)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(ad))
}

#[derive(Deserialize)]
pub struct ServeQ {
    pub limit: Option<i64>,
}

/// Picks eligible active campaigns and records an impression for each.
pub async fn serve(
    State(st): State<AppState>,
    Query(q): Query<ServeQ>,
) -> AppResult<Json<Vec<Value>>> {
    let rows: Vec<(Uuid, Uuid, Uuid, String, String, Option<String>, String, i64, serde_json::Value, String, String, Uuid)> = sqlx::query_as(
        "WITH picked AS (
            SELECT a.id FROM ad_campaigns a
            JOIN products p ON p.id = a.product_id
            WHERE a.status='active' AND p.status = 'active' AND p.review_status = 'approved' AND p.stock > 0
              AND a.spent_cents + a.cpc_cents <= a.budget_cents
              AND (a.starts_at IS NULL OR a.starts_at <= now())
              AND (a.ends_at IS NULL OR a.ends_at > now())
            ORDER BY random() * a.cpc_cents DESC
            LIMIT $1
         ), upd AS (
            UPDATE ad_campaigns a SET impressions = impressions + 1 FROM picked WHERE a.id = picked.id
            RETURNING a.*
         )
         SELECT upd.id, upd.shop_id, upd.product_id, upd.headline, upd.body, upd.image_url,
                p.name, p.price_cents, p.images, s.name, s.slug, p.shop_id
         FROM upd JOIN products p ON p.id = upd.product_id JOIN shops s ON s.id = upd.shop_id",
    )
    .bind(q.limit.unwrap_or(3).clamp(1, 10))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, shop_id, product_id, headline, body, image, pname, price, images, sname, sslug, supplier)| {
                json!({
                    "id": id, "shop_id": shop_id, "product_id": product_id, "headline": headline, "body": body,
                    "image_url": image.or_else(|| images.get(0).and_then(|v| v.as_str()).map(String::from)),
                    "product_name": pname, "price_cents": price, "shop_name": sname, "shop_slug": sslug,
                    "via_shop_id": if shop_id != supplier { Some(shop_id) } else { None }
                })
            })
            .collect(),
    ))
}

/// Records a click and charges CPC. Campaign ends automatically when budget is exhausted.
pub async fn click(State(st): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let row: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "UPDATE ad_campaigns SET
            clicks = clicks + 1,
            spent_cents = spent_cents + cpc_cents,
            status = CASE WHEN spent_cents + 2 * cpc_cents > budget_cents THEN 'ended' ELSE status END
         WHERE id=$1 AND status='active' AND spent_cents + cpc_cents <= budget_cents
         RETURNING product_id, shop_id, (SELECT shop_id FROM products WHERE id = product_id)",
    )
    .bind(id)
    .fetch_optional(&st.db)
    .await?;
    let (product_id, shop_id, supplier) = row.ok_or(AppError::NotFound)?;
    Ok(Json(json!({
        "product_id": product_id,
        "via_shop_id": if shop_id != supplier { Some(shop_id) } else { None }
    })))
}
