use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{Content, Paging},
    routes::owned_shop,
    AppState,
};

pub const KINDS: [&str; 5] = ["post", "caption", "video_script", "description", "ad_copy"];

#[derive(Deserialize)]
pub struct CreateContent {
    pub product_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub media_url: Option<String>,
    pub ai_generated: Option<bool>,
    pub status: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateContent {
    pub title: Option<String>,
    pub body: Option<String>,
    pub media_url: Option<String>,
    pub status: Option<String>,
}

fn check(kind: Option<&str>, status: Option<&str>) -> AppResult<()> {
    if let Some(k) = kind {
        if !KINDS.contains(&k) {
            return Err(AppError::bad(format!("kind must be one of {KINDS:?}")));
        }
    }
    if !matches!(status, None | Some("draft" | "published")) {
        return Err(AppError::bad("status must be draft or published"));
    }
    Ok(())
}

pub async fn public_feed(
    State(st): State<AppState>,
    Query(p): Query<Paging>,
) -> AppResult<Json<Vec<Value>>> {
    let rows: Vec<(
        Uuid,
        String,
        String,
        String,
        Option<String>,
        chrono::DateTime<chrono::Utc>,
        Uuid,
        String,
        String,
        Option<Uuid>,
        Option<String>,
        Option<i64>,
        Option<Uuid>,
    )> = sqlx::query_as(
        "SELECT c.id, c.kind, c.title, c.body, c.media_url, c.created_at, s.id, s.name, s.slug,
                    p.id, p.name, p.price_cents, p.shop_id
             FROM contents c JOIN shops s ON s.id = c.shop_id
             LEFT JOIN products p ON p.id = c.product_id AND p.status = 'active' AND p.review_status = 'approved'
             WHERE c.status='published' AND c.kind IN ('post','video_script','caption')
             ORDER BY c.created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(p.limit())
    .bind(p.offset())
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, kind, title, body, media, at, sid, sname, sslug, pid, pname, price, supplier)| {
                json!({
                    "id": id, "kind": kind, "title": title, "body": body, "media_url": media, "created_at": at,
                    "shop": { "id": sid, "name": sname, "slug": sslug },
                    "product": pid.map(|pid| json!({
                        "id": pid, "name": pname, "price_cents": price,
                        // creator links carry attribution so they earn commission
                        "via_shop_id": if supplier != Some(sid) { Some(sid) } else { None }
                    }))
                })
            })
            .collect(),
    ))
}

pub async fn list(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
) -> AppResult<Json<Vec<Content>>> {
    owned_shop(&st, shop_id, &user).await?;
    Ok(Json(
        sqlx::query_as("SELECT * FROM contents WHERE shop_id=$1 ORDER BY created_at DESC")
            .bind(shop_id)
            .fetch_all(&st.db)
            .await?,
    ))
}

pub async fn create(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<CreateContent>,
) -> AppResult<Json<Content>> {
    owned_shop(&st, shop_id, &user).await?;
    check(Some(&req.kind), req.status.as_deref())?;
    if req.title.trim().is_empty() || req.body.trim().is_empty() {
        return Err(AppError::bad("title and body are required"));
    }
    insert(&st, shop_id, &req).await.map(Json)
}

pub async fn insert(st: &AppState, shop_id: Uuid, req: &CreateContent) -> AppResult<Content> {
    Ok(sqlx::query_as(
        "INSERT INTO contents (shop_id, product_id, kind, title, body, media_url, ai_generated, status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *",
    )
    .bind(shop_id)
    .bind(req.product_id)
    .bind(&req.kind)
    .bind(req.title.trim())
    .bind(&req.body)
    .bind(&req.media_url)
    .bind(req.ai_generated.unwrap_or(false))
    .bind(req.status.clone().unwrap_or_else(|| "draft".into()))
    .fetch_one(&st.db)
    .await?)
}

async fn content_shop(st: &AppState, id: Uuid) -> AppResult<Uuid> {
    sqlx::query_scalar("SELECT shop_id FROM contents WHERE id=$1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateContent>,
) -> AppResult<Json<Content>> {
    owned_shop(&st, content_shop(&st, id).await?, &user).await?;
    check(None, req.status.as_deref())?;
    Ok(Json(
        sqlx::query_as(
            "UPDATE contents SET title=COALESCE($2,title), body=COALESCE($3,body),
                 media_url=COALESCE($4,media_url), status=COALESCE($5,status)
             WHERE id=$1 RETURNING *",
        )
        .bind(id)
        .bind(req.title)
        .bind(req.body)
        .bind(req.media_url)
        .bind(req.status)
        .fetch_one(&st.db)
        .await?,
    ))
}

pub async fn remove(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, content_shop(&st, id).await?, &user).await?;
    sqlx::query("DELETE FROM contents WHERE id=$1")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
