use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{InventoryMovement, Product},
    routes::{owned_product, owned_shop},
    AppState,
};

#[derive(Deserialize)]
pub struct AdjustReq {
    pub delta: i32,
    /// restock | adjust | return
    pub reason: String,
    pub note: Option<String>,
}

pub async fn adjust_stock(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AdjustReq>,
) -> AppResult<Json<Product>> {
    owned_product(&st, id, &user).await?;
    if !matches!(req.reason.as_str(), "restock" | "adjust" | "return") {
        return Err(AppError::bad("reason must be restock, adjust or return"));
    }
    if req.delta == 0 {
        return Err(AppError::bad("delta must be non-zero"));
    }
    let mut tx = st.db.begin().await?;
    let p: Option<Product> = sqlx::query_as(
        "UPDATE products SET stock = stock + $2, updated_at = now()
         WHERE id = $1 AND stock + $2 >= 0 RETURNING *",
    )
    .bind(id)
    .bind(req.delta)
    .fetch_optional(&mut *tx)
    .await?;
    let p = p.ok_or_else(|| AppError::bad("stock cannot go below zero"))?;
    sqlx::query(
        "INSERT INTO inventory_movements (product_id, delta, stock_after, reason, note, created_by)
         VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(id)
    .bind(req.delta)
    .bind(p.stock)
    .bind(&req.reason)
    .bind(req.note.unwrap_or_default())
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(p))
}

#[derive(Deserialize)]
pub struct MovQ {
    pub product_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn movements(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<MovQ>,
) -> AppResult<Json<Vec<InventoryMovement>>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows = sqlx::query_as(
        "SELECT m.id, m.product_id, p.name AS product_name, m.delta, m.stock_after, m.reason,
                m.ref_order_id, m.note, m.created_at
         FROM inventory_movements m JOIN products p ON p.id = m.product_id
         WHERE p.shop_id = $1 AND ($2::uuid IS NULL OR m.product_id = $2)
         ORDER BY m.created_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(shop_id)
    .bind(q.product_id)
    .bind(q.limit.unwrap_or(50).clamp(1, 200))
    .bind(q.offset.unwrap_or(0).max(0))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

pub async fn low_stock(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
) -> AppResult<Json<Vec<Product>>> {
    owned_shop(&st, shop_id, &user).await?;
    let rows = sqlx::query_as(
        "SELECT * FROM products WHERE shop_id = $1 AND status <> 'archived' AND stock <= low_stock_threshold
         ORDER BY stock ASC",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}
