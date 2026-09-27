//! Product review (moderation) workflow + admin console endpoints.
//!
//! State machine (`products.review_status`):
//!   not_submitted ──publish──▶ pending ──approve──▶ approved
//!        ▲                        │                    │
//!        └──── edit while draft ──┤◀──── content edit ─┘   (REVIEW_ON_EDIT)
//!                                 └──reject──▶ rejected ──fix & publish──▶ pending
//! A product is visible to shoppers only when `status = 'active' AND review_status = 'approved'`.
//! `PRODUCT_REVIEW=off` or a shop with `auto_approve` skips the queue (logged as auto_approved).

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    auth::{AdminUser, AuthUser},
    error::{AppError, AppResult},
    models::{Product, Shop},
    routes::{media::public_gallery, owned_product},
    AppState,
};

async fn log(tx: &mut Transaction<'_, Postgres>, product_id: Uuid, actor: Option<Uuid>, action: &str, note: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO product_reviews (product_id, actor_id, action, note) VALUES ($1,$2,$3,$4)")
        .bind(product_id)
        .bind(actor)
        .bind(action)
        .bind(note)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Apply review rules after a product was created/updated inside `tx`.
/// `content_changed`: name, description, marketplace category or gallery changed.
pub async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    st: &AppState,
    product_id: Uuid,
    actor: Uuid,
    content_changed: bool,
) -> AppResult<Product> {
    let p: Product = sqlx::query_as("SELECT * FROM products WHERE id = $1 FOR UPDATE")
        .bind(product_id)
        .fetch_one(&mut **tx)
        .await?;
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1")
        .bind(p.shop_id)
        .fetch_one(&mut **tx)
        .await?;
    let auto = !st.cfg.review_required || shop.auto_approve;

    let rs = p.review_status.as_str();
    let (next, action): (&str, Option<&str>) = if p.status == "active" {
        match rs {
            "not_submitted" | "rejected" if auto => ("approved", Some("auto_approved")),
            "not_submitted" => ("pending", Some("submitted")),
            "rejected" => ("pending", Some("resubmitted")),
            "approved" if content_changed && st.cfg.review_on_edit && !auto => ("pending", Some("resubmitted")),
            _ => (rs, None),
        }
    } else {
        match rs {
            // Unpublished edits to an approved product must be reviewed again before going live.
            "approved" if content_changed && st.cfg.review_on_edit && !auto => ("not_submitted", None),
            // Taking a pending product back to draft withdraws it from the queue.
            "pending" => ("not_submitted", None),
            _ => (rs, None),
        }
    };

    if next == rs && action.is_none() {
        return Ok(p);
    }
    let publishing = matches!(rs, "not_submitted" | "rejected") && matches!(next, "pending" | "approved");
    if publishing && p.category_id.is_none() {
        return Err(AppError::bad("choose a marketplace category before publishing"));
    }
    let p: Product = sqlx::query_as(
        "UPDATE products SET review_status = $2,
            submitted_at = CASE WHEN $2 = 'pending' THEN now() ELSE submitted_at END,
            reviewed_at = CASE WHEN $2 = 'approved' THEN now() ELSE reviewed_at END,
            reviewed_by = CASE WHEN $2 = 'approved' THEN NULL ELSE reviewed_by END,
            review_note = CASE WHEN $2 IN ('pending','approved') THEN '' ELSE review_note END
         WHERE id = $1 RETURNING *",
    )
    .bind(product_id)
    .bind(next)
    .fetch_one(&mut **tx)
    .await?;
    if let Some(a) = action {
        log(tx, product_id, Some(actor), a, "").await?;
    }
    Ok(p)
}

// ---------------------------------------------------------------------------
// Seller
// ---------------------------------------------------------------------------

/// Publish + submit for review in one step.
pub async fn submit(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Product>> {
    owned_product(&st, id, &user).await?;
    let mut tx = st.db.begin().await?;
    sqlx::query("UPDATE products SET status = 'active', updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let p = apply(&mut tx, &st, id, user.id, false).await?;
    tx.commit().await?;
    Ok(Json(p))
}

pub async fn history(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Vec<Value>>> {
    owned_product(&st, id, &user).await?;
    history_of(&st, id, false).await.map(Json)
}

async fn history_of(st: &AppState, id: Uuid, include_actor: bool) -> AppResult<Vec<Value>> {
    let rows: Vec<(String, String, DateTime<Utc>, Option<String>)> = sqlx::query_as(
        "SELECT r.action, r.note, r.created_at, u.display_name
         FROM product_reviews r LEFT JOIN users u ON u.id = r.actor_id
         WHERE r.product_id = $1 ORDER BY r.created_at DESC LIMIT 50",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(action, note, at, actor)| {
            let actor = if include_actor || !matches!(action.as_str(), "approved" | "rejected") { actor } else { Some("Marketplace team".into()) };
            json!({ "action": action, "note": note, "created_at": at, "actor": actor })
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Admin
// ---------------------------------------------------------------------------

pub async fn overview(State(st): State<AppState>, _a: AdminUser) -> AppResult<Json<Value>> {
    let (pending, rejected, approved, live): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE review_status='pending'),
                COUNT(*) FILTER (WHERE review_status='rejected'),
                COUNT(*) FILTER (WHERE review_status='approved'),
                COUNT(*) FILTER (WHERE review_status='approved' AND status='active')
         FROM products",
    )
    .fetch_one(&st.db)
    .await?;
    let (shops, users, categories): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM shops), (SELECT COUNT(*) FROM users),
                (SELECT COUNT(*) FROM categories WHERE shop_id IS NULL)",
    )
    .fetch_one(&st.db)
    .await?;
    let oldest: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT MIN(submitted_at) FROM products WHERE review_status='pending'")
            .fetch_one(&st.db)
            .await?;
    Ok(Json(json!({
        "pending": pending, "rejected": rejected, "approved": approved, "live": live,
        "shops": shops, "users": users, "categories": categories, "oldest_pending_at": oldest,
        "review_required": st.cfg.review_required, "review_on_edit": st.cfg.review_on_edit
    })))
}

#[derive(Deserialize)]
pub struct QueueQ {
    pub review_status: Option<String>,
    pub q: Option<String>,
    pub shop_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn queue(State(st): State<AppState>, _a: AdminUser, Query(q): Query<QueueQ>) -> AppResult<Json<Vec<Value>>> {
    type Row = (Uuid, String, String, i64, Value, String, String, String, Option<DateTime<Utc>>, DateTime<Utc>, Uuid, String, String, Option<String>, i32);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT p.id, p.name, p.sku, p.price_cents, p.images, p.status, p.review_status, p.review_note,
                p.submitted_at, p.updated_at, s.id, s.name, s.currency, c.name, p.stock
         FROM products p
         JOIN shops s ON s.id = p.shop_id
         LEFT JOIN categories c ON c.id = p.category_id
         WHERE ($1::text IS NULL OR p.review_status = $1)
           AND ($2::text IS NULL OR p.name ILIKE '%'||$2||'%' OR s.name ILIKE '%'||$2||'%' OR p.sku ILIKE '%'||$2||'%')
           AND ($3::uuid IS NULL OR p.shop_id = $3)
           AND p.review_status <> 'not_submitted'
         ORDER BY CASE WHEN p.review_status = 'pending' THEN p.submitted_at END ASC NULLS LAST, p.updated_at DESC
         LIMIT $4 OFFSET $5",
    )
    .bind(q.review_status.filter(|s| !s.is_empty()))
    .bind(q.q.filter(|s| !s.trim().is_empty()))
    .bind(q.shop_id)
    .bind(q.limit.unwrap_or(50).clamp(1, 200))
    .bind(q.offset.unwrap_or(0).max(0))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, name, sku, price, images, status, rs, note, submitted, updated, sid, sname, cur, cat, stock)| {
                json!({
                    "id": id, "name": name, "sku": sku, "price_cents": price, "cover": images.get(0),
                    "status": status, "review_status": rs, "review_note": note, "submitted_at": submitted,
                    "updated_at": updated, "shop": { "id": sid, "name": sname }, "currency": cur,
                    "category": cat, "stock": stock
                })
            })
            .collect(),
    ))
}

pub async fn detail(State(st): State<AppState>, _a: AdminUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let p: Product = sqlx::query_as("SELECT * FROM products WHERE id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1").bind(p.shop_id).fetch_one(&st.db).await?;
    let (owner_email, owner_name): (String, String) =
        sqlx::query_as("SELECT email, display_name FROM users WHERE id = $1")
            .bind(shop.owner_id)
            .fetch_one(&st.db)
            .await?;
    let category_path: Option<String> = match p.category_id {
        Some(cid) => sqlx::query_scalar(
            "WITH RECURSIVE up AS (
                SELECT id, parent_id, name, 0 AS d FROM categories WHERE id = $1
                UNION ALL SELECT c.id, c.parent_id, c.name, up.d + 1 FROM categories c JOIN up ON c.id = up.parent_id
             ) SELECT string_agg(name, ' › ' ORDER BY d DESC) FROM up",
        )
        .bind(cid)
        .fetch_one(&st.db)
        .await?,
        None => None,
    };
    let (shop_live, shop_rejected): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE review_status='approved'), COUNT(*) FILTER (WHERE review_status='rejected')
         FROM products WHERE shop_id = $1",
    )
    .bind(shop.id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({
        "product": p,
        "shop": shop,
        "owner": { "email": owner_email, "name": owner_name },
        "shop_stats": { "approved": shop_live, "rejected": shop_rejected },
        "category_path": category_path,
        "media": public_gallery(&st, id).await?,
        "history": history_of(&st, id, true).await?,
    })))
}

#[derive(Deserialize)]
pub struct DecisionReq {
    /// approve | reject
    pub action: String,
    pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct BulkDecisionReq {
    pub ids: Vec<Uuid>,
    pub action: String,
    pub note: Option<String>,
}

async fn decide_one(st: &AppState, admin: Uuid, id: Uuid, action: &str, note: &str) -> AppResult<Product> {
    let next = match action {
        "approve" => "approved",
        "reject" => {
            if note.trim().is_empty() {
                return Err(AppError::bad("a reason is required when rejecting, so the seller knows what to fix"));
            }
            "rejected"
        }
        _ => return Err(AppError::bad("action must be approve or reject")),
    };
    let mut tx = st.db.begin().await?;
    let p: Product = sqlx::query_as(
        "UPDATE products SET review_status = $2, review_note = $3, reviewed_at = now(), reviewed_by = $4
         WHERE id = $1 AND review_status <> 'not_submitted' RETURNING *",
    )
    .bind(id)
    .bind(next)
    .bind(note.trim())
    .bind(admin)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::bad("product not found or not submitted for review"))?;
    log(&mut tx, id, Some(admin), next, note.trim()).await?;
    tx.commit().await?;
    Ok(p)
}

pub async fn decide(
    State(st): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<DecisionReq>,
) -> AppResult<Json<Product>> {
    Ok(Json(decide_one(&st, admin.id, id, &req.action, req.note.as_deref().unwrap_or("")).await?))
}

pub async fn decide_bulk(
    State(st): State<AppState>,
    AdminUser(admin): AdminUser,
    Json(req): Json<BulkDecisionReq>,
) -> AppResult<Json<Value>> {
    if req.ids.is_empty() || req.ids.len() > 100 {
        return Err(AppError::bad("select between 1 and 100 products"));
    }
    let note = req.note.unwrap_or_default();
    let (mut ok, mut failed) = (0, Vec::new());
    for id in req.ids {
        match decide_one(&st, admin.id, id, &req.action, &note).await {
            Ok(_) => ok += 1,
            Err(e) => failed.push(json!({ "id": id, "message": e.to_string() })),
        }
    }
    Ok(Json(json!({ "updated": ok, "failed": failed })))
}

#[derive(Deserialize)]
pub struct ShopsQ {
    pub q: Option<String>,
}

pub async fn shops(State(st): State<AppState>, _a: AdminUser, Query(q): Query<ShopsQ>) -> AppResult<Json<Vec<Value>>> {
    type Row = (Uuid, String, String, String, bool, DateTime<Utc>, String, i64, i64, i64);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT s.id, s.name, s.slug, s.kind, s.auto_approve, s.created_at, u.email,
                COUNT(p.id), COUNT(p.id) FILTER (WHERE p.review_status='pending'),
                COUNT(p.id) FILTER (WHERE p.review_status='rejected')
         FROM shops s JOIN users u ON u.id = s.owner_id
         LEFT JOIN products p ON p.shop_id = s.id
         WHERE ($1::text IS NULL OR s.name ILIKE '%'||$1||'%' OR u.email ILIKE '%'||$1||'%')
         GROUP BY s.id, u.email ORDER BY s.created_at DESC LIMIT 200",
    )
    .bind(q.q.filter(|s| !s.trim().is_empty()))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, name, slug, kind, auto, at, email, total, pending, rejected)| {
                json!({ "id": id, "name": name, "slug": slug, "kind": kind, "auto_approve": auto, "created_at": at,
                        "owner_email": email, "products": total, "pending": pending, "rejected": rejected })
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct ShopPatch {
    pub auto_approve: Option<bool>,
}

pub async fn patch_shop(
    State(st): State<AppState>,
    _a: AdminUser,
    Path(id): Path<Uuid>,
    Json(req): Json<ShopPatch>,
) -> AppResult<Json<Shop>> {
    Ok(Json(
        sqlx::query_as("UPDATE shops SET auto_approve = COALESCE($2, auto_approve) WHERE id = $1 RETURNING *")
            .bind(id)
            .bind(req.auto_approve)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?,
    ))
}
