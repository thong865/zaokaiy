//! Category trees.
//!
//! * Marketplace categories (`shop_id IS NULL`) — managed by platform admins; every product
//!   picks one (required before publishing) and shoppers browse by them.
//! * Shop categories (`shop_id = <shop>`) — each seller's own category / sub-category tree used
//!   to organise their storefront.
//!
//! Trees are at most `MAX_DEPTH` levels deep (category → sub-category → sub-sub-category).

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::{AdminUser, AuthUser},
    error::{AppError, AppResult},
    routes::owned_shop,
    AppState,
};

pub const MAX_DEPTH: i32 = 3;

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct Category {
    pub id: Uuid,
    pub shop_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub image_url: Option<String>,
    pub position: i32,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Category with tree info, ready for pickers and tree views (pre-order, siblings by position).
#[derive(Debug, Serialize, FromRow)]
pub struct CategoryNode {
    #[sqlx(flatten)]
    #[serde(flatten)]
    pub category: Category,
    pub depth: i32,
    pub path: String,
    #[sqlx(skip)]
    pub product_count: i64,
    #[sqlx(skip)]
    pub total_count: i64,
    #[sqlx(skip)]
    pub child_count: i64,
}

pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').chars().take(60).collect::<String>();
    if out.len() < 2 {
        // Non-Latin names (Thai, Lao, Chinese…) get a short random slug.
        format!("c-{}", &Uuid::new_v4().simple().to_string()[..6])
    } else {
        out
    }
}

/// Loads a scope's tree in display order with product counts.
/// `visible_only`: only active categories and only live (approved + active) products are counted.
pub async fn load_tree(st: &AppState, shop_id: Option<Uuid>, visible_only: bool) -> AppResult<Vec<CategoryNode>> {
    let mut nodes: Vec<CategoryNode> = sqlx::query_as(
        "WITH RECURSIVE t AS (
            SELECT c.*, 0 AS depth, c.name::text AS path,
                   ARRAY[lpad(c.position::text, 6, '0') || c.name || c.id::text] AS sort
            FROM categories c
            WHERE c.parent_id IS NULL AND c.shop_id IS NOT DISTINCT FROM $1 AND (NOT $2 OR c.active)
            UNION ALL
            SELECT c.*, t.depth + 1, t.path || ' › ' || c.name,
                   t.sort || (lpad(c.position::text, 6, '0') || c.name || c.id::text)
            FROM categories c JOIN t ON c.parent_id = t.id
            WHERE (NOT $2 OR c.active)
         )
         SELECT id, shop_id, parent_id, name, slug, description, image_url, position, active,
                created_at, updated_at, depth, path
         FROM t ORDER BY sort",
    )
    .bind(shop_id)
    .bind(visible_only)
    .fetch_all(&st.db)
    .await?;

    let col = if shop_id.is_some() { "shop_category_id" } else { "category_id" };
    let counts: Vec<(Uuid, i64)> = sqlx::query_as(&format!(
        "SELECT {col}, COUNT(*) FROM products
         WHERE {col} IS NOT NULL
           AND ($1::uuid IS NULL OR shop_id = $1)
           AND (NOT $2 OR (status = 'active' AND review_status = 'approved'))
           AND status <> 'archived'
         GROUP BY {col}"
    ))
    .bind(shop_id)
    .bind(visible_only)
    .fetch_all(&st.db)
    .await?;
    let direct: HashMap<Uuid, i64> = counts.into_iter().collect();

    // Roll counts up to ancestors (nodes are pre-ordered, so walk in reverse).
    let parent_of: HashMap<Uuid, Option<Uuid>> = nodes.iter().map(|n| (n.category.id, n.category.parent_id)).collect();
    let mut total: HashMap<Uuid, i64> = HashMap::new();
    let mut children: HashMap<Uuid, i64> = HashMap::new();
    for n in nodes.iter().rev() {
        let id = n.category.id;
        let t = total.get(&id).copied().unwrap_or(0) + direct.get(&id).copied().unwrap_or(0);
        total.insert(id, t);
        if let Some(Some(p)) = parent_of.get(&id) {
            *total.entry(*p).or_default() += t;
            *children.entry(*p).or_default() += 1;
        }
    }
    for n in nodes.iter_mut() {
        let id = n.category.id;
        n.product_count = direct.get(&id).copied().unwrap_or(0);
        n.total_count = total.get(&id).copied().unwrap_or(0);
        n.child_count = children.get(&id).copied().unwrap_or(0);
    }
    Ok(nodes)
}

async fn load(st: &AppState, id: Uuid) -> AppResult<Category> {
    sqlx::query_as("SELECT * FROM categories WHERE id = $1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)
}

async fn is_admin(st: &AppState, user: &AuthUser) -> AppResult<bool> {
    let role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_optional(&st.db)
        .await?;
    Ok(role.as_deref() == Some("admin"))
}

/// Marketplace categories need an admin; shop categories need the shop owner.
async fn authorize(st: &AppState, cat: &Category, user: &AuthUser) -> AppResult<()> {
    match cat.shop_id {
        None if is_admin(st, user).await? => Ok(()),
        None => Err(AppError::Forbidden),
        Some(shop) => owned_shop(st, shop, user).await.map(|_| ()),
    }
}

/// Depth of a node (0 = root).
async fn depth_of(st: &AppState, id: Uuid) -> AppResult<i32> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE up AS (
            SELECT id, parent_id, 0 AS d FROM categories WHERE id = $1
            UNION ALL SELECT c.id, c.parent_id, up.d + 1 FROM categories c JOIN up ON c.id = up.parent_id
         ) SELECT MAX(d) FROM up",
    )
    .bind(id)
    .fetch_one(&st.db)
    .await?)
}

/// Height of the subtree below a node (0 = leaf).
async fn height_of(st: &AppState, id: Uuid) -> AppResult<i32> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE down AS (
            SELECT id, 0 AS h FROM categories WHERE id = $1
            UNION ALL SELECT c.id, down.h + 1 FROM categories c JOIN down ON c.parent_id = down.id
         ) SELECT MAX(h) FROM down",
    )
    .bind(id)
    .fetch_one(&st.db)
    .await?)
}

async fn is_descendant(st: &AppState, ancestor: Uuid, node: Uuid) -> AppResult<bool> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE down AS (
            SELECT id FROM categories WHERE id = $1
            UNION ALL SELECT c.id FROM categories c JOIN down ON c.parent_id = down.id
         ) SELECT EXISTS (SELECT 1 FROM down WHERE id = $2)",
    )
    .bind(ancestor)
    .bind(node)
    .fetch_one(&st.db)
    .await?)
}

async fn check_parent(st: &AppState, scope: Option<Uuid>, parent: Uuid, extra_height: i32) -> AppResult<()> {
    let p = load(st, parent).await.map_err(|_| AppError::bad("parent category not found"))?;
    if p.shop_id != scope {
        return Err(AppError::bad("parent category belongs to a different tree"));
    }
    if depth_of(st, parent).await? + 1 + extra_height >= MAX_DEPTH {
        return Err(AppError::bad(format!("categories can be nested at most {MAX_DEPTH} levels deep")));
    }
    Ok(())
}

async fn unique_slug(st: &AppState, scope: Option<Uuid>, base: &str, except: Option<Uuid>) -> AppResult<String> {
    for i in 0..50 {
        let candidate = if i == 0 { base.to_string() } else { format!("{base}-{}", i + 1) };
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM categories
             WHERE shop_id IS NOT DISTINCT FROM $1 AND slug = $2 AND ($3::uuid IS NULL OR id <> $3))",
        )
        .bind(scope)
        .bind(&candidate)
        .bind(except)
        .fetch_one(&st.db)
        .await?;
        if !taken {
            return Ok(candidate);
        }
    }
    Err(AppError::Conflict("could not find a free slug".into()))
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Public marketplace tree (active categories, live product counts).
pub async fn public_tree(State(st): State<AppState>) -> AppResult<Json<Vec<CategoryNode>>> {
    Ok(Json(load_tree(&st, None, true).await?))
}

pub async fn admin_tree(State(st): State<AppState>, _admin: AdminUser) -> AppResult<Json<Vec<CategoryNode>>> {
    Ok(Json(load_tree(&st, None, false).await?))
}

pub async fn shop_tree(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Vec<CategoryNode>>> {
    owned_shop(&st, shop_id, &user).await?;
    Ok(Json(load_tree(&st, Some(shop_id), false).await?))
}

#[derive(Deserialize)]
pub struct CreateReq {
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub active: Option<bool>,
}

async fn create(st: &AppState, scope: Option<Uuid>, req: CreateReq) -> AppResult<Category> {
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::bad("name is required (max 80 characters)"));
    }
    if let Some(p) = req.parent_id {
        check_parent(st, scope, p, 0).await?;
    }
    let base = slugify(req.slug.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or(name));
    let slug = unique_slug(st, scope, &base, None).await?;
    let position: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM categories
         WHERE shop_id IS NOT DISTINCT FROM $1 AND parent_id IS NOT DISTINCT FROM $2",
    )
    .bind(scope)
    .bind(req.parent_id)
    .fetch_one(&st.db)
    .await?;
    Ok(sqlx::query_as(
        "INSERT INTO categories (shop_id, parent_id, name, slug, description, image_url, position, active)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *",
    )
    .bind(scope)
    .bind(req.parent_id)
    .bind(name)
    .bind(slug)
    .bind(req.description.unwrap_or_default())
    .bind(req.image_url.filter(|u| !u.is_empty()))
    .bind(position)
    .bind(req.active.unwrap_or(true))
    .fetch_one(&st.db)
    .await?)
}

pub async fn create_shop_category(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<CreateReq>,
) -> AppResult<Json<Category>> {
    owned_shop(&st, shop_id, &user).await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories WHERE shop_id = $1")
        .bind(shop_id)
        .fetch_one(&st.db)
        .await?;
    if count >= 200 {
        return Err(AppError::bad("a shop can have at most 200 categories"));
    }
    Ok(Json(create(&st, Some(shop_id), req).await?))
}

pub async fn create_marketplace_category(
    State(st): State<AppState>,
    _admin: AdminUser,
    Json(req): Json<CreateReq>,
) -> AppResult<Json<Category>> {
    Ok(Json(create(&st, None, req).await?))
}

#[derive(Deserialize)]
pub struct UpdateReq {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub active: Option<bool>,
    /// Absent = unchanged, `null` = move to top level, uuid = move under that category.
    #[serde(default, deserialize_with = "double_option")]
    pub parent_id: Option<Option<Uuid>>,
}

fn double_option<'de, D>(d: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::<Uuid>::deserialize(d)?))
}

pub async fn update(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateReq>,
) -> AppResult<Json<Category>> {
    let cat = load(&st, id).await?;
    authorize(&st, &cat, &user).await?;

    let mut parent = cat.parent_id;
    let mut position = cat.position;
    if let Some(new_parent) = req.parent_id {
        if new_parent != cat.parent_id {
            if let Some(p) = new_parent {
                if p == id || is_descendant(&st, id, p).await? {
                    return Err(AppError::bad("a category cannot be moved inside itself"));
                }
                check_parent(&st, cat.shop_id, p, height_of(&st, id).await?).await?;
            }
            parent = new_parent;
            position = sqlx::query_scalar(
                "SELECT COALESCE(MAX(position), 0) + 1 FROM categories
                 WHERE shop_id IS NOT DISTINCT FROM $1 AND parent_id IS NOT DISTINCT FROM $2",
            )
            .bind(cat.shop_id)
            .bind(parent)
            .fetch_one(&st.db)
            .await?;
        }
    }
    let name = match req.name.as_deref().map(str::trim) {
        Some("") => return Err(AppError::bad("name cannot be empty")),
        Some(n) if n.chars().count() > 80 => return Err(AppError::bad("name is too long")),
        Some(n) => n.to_string(),
        None => cat.name.clone(),
    };
    let slug = match req.slug.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => unique_slug(&st, cat.shop_id, &slugify(s), Some(id)).await?,
        None => cat.slug.clone(),
    };

    let updated: Category = sqlx::query_as(
        "UPDATE categories SET name=$2, slug=$3, description=COALESCE($4, description),
            image_url = CASE WHEN $5::text IS NULL THEN image_url WHEN $5 = '' THEN NULL ELSE $5 END,
            active=COALESCE($6, active), parent_id=$7, position=$8, updated_at=now()
         WHERE id=$1 RETURNING *",
    )
    .bind(id)
    .bind(name)
    .bind(slug)
    .bind(req.description)
    .bind(req.image_url)
    .bind(req.active)
    .bind(parent)
    .bind(position)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(updated))
}

#[derive(Deserialize)]
pub struct ReorderReq {
    /// Sibling ids in the desired order (all must share the same parent).
    pub ids: Vec<Uuid>,
}

pub async fn reorder(State(st): State<AppState>, user: AuthUser, Json(req): Json<ReorderReq>) -> AppResult<Json<Value>> {
    let Some(first) = req.ids.first() else { return Err(AppError::bad("ids required")) };
    let cat = load(&st, *first).await?;
    authorize(&st, &cat, &user).await?;
    let same: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM categories
         WHERE id = ANY($1) AND shop_id IS NOT DISTINCT FROM $2 AND parent_id IS NOT DISTINCT FROM $3",
    )
    .bind(&req.ids)
    .bind(cat.shop_id)
    .bind(cat.parent_id)
    .fetch_one(&st.db)
    .await?;
    if same as usize != req.ids.len() {
        return Err(AppError::bad("all categories must share the same parent"));
    }
    // Renumber every sibling: the given ids first (in order), then the rest in their old order.
    sqlx::query(
        "WITH given AS (
            SELECT id, ord FROM unnest($1::uuid[]) WITH ORDINALITY AS t(id, ord)
         ), ranked AS (
            SELECT c.id,
                   ROW_NUMBER() OVER (ORDER BY (g.ord IS NULL), g.ord, c.position, c.name, c.id) AS pos
            FROM categories c LEFT JOIN given g ON g.id = c.id
            WHERE c.shop_id IS NOT DISTINCT FROM $2 AND c.parent_id IS NOT DISTINCT FROM $3
         )
         UPDATE categories c SET position = r.pos::int, updated_at = now()
         FROM ranked r WHERE c.id = r.id",
    )
    .bind(&req.ids)
    .bind(cat.shop_id)
    .bind(cat.parent_id)
    .execute(&st.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DeleteQ {
    /// Marketplace only: move products to this category before deleting.
    pub reassign_to: Option<Uuid>,
}

pub async fn remove(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<DeleteQ>,
) -> AppResult<Json<Value>> {
    let cat = load(&st, id).await?;
    authorize(&st, &cat, &user).await?;
    let children: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories WHERE parent_id = $1")
        .bind(id)
        .fetch_one(&st.db)
        .await?;
    if children > 0 {
        return Err(AppError::Conflict(format!(
            "has {children} sub-categor{} — move or delete them first",
            if children == 1 { "y" } else { "ies" }
        )));
    }
    let mut tx = st.db.begin().await?;
    let moved = if cat.shop_id.is_some() {
        // Shop categories: products simply become uncategorised in the storefront.
        sqlx::query("UPDATE products SET shop_category_id = NULL WHERE shop_category_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
    } else {
        let used: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM products WHERE category_id = $1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        match (used, q.reassign_to) {
            (0, _) => 0,
            (_, None) => {
                return Err(AppError::Conflict(format!(
                    "{used} product(s) use this category — reassign them or deactivate the category instead"
                )))
            }
            (_, Some(target)) => {
                let t = load(&st, target).await?;
                if t.shop_id.is_some() || t.id == id {
                    return Err(AppError::bad("reassign_to must be another marketplace category"));
                }
                sqlx::query("UPDATE products SET category_id = $2, category = $3, updated_at = now() WHERE category_id = $1")
                    .bind(id)
                    .bind(t.id)
                    .bind(&t.slug)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected()
            }
        }
    };
    sqlx::query("DELETE FROM categories WHERE id = $1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "deleted": true, "products_updated": moved })))
}

/// Resolve a marketplace category slug to itself + all descendant ids (for catalog filtering).
pub async fn subtree_ids_by_slug(st: &AppState, slug: &str) -> AppResult<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "WITH RECURSIVE down AS (
            SELECT id FROM categories WHERE shop_id IS NULL AND slug = $1
            UNION ALL SELECT c.id FROM categories c JOIN down ON c.parent_id = down.id
         ) SELECT id FROM down",
    )
    .bind(slug)
    .fetch_all(&st.db)
    .await?)
}
