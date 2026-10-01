//! Ownership checks shared by every core.

use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::Shop,
    AppState,
};

/// Load a shop and ensure the caller may act on it for the current request: the owner and admins
/// always; staff when their role has the permission the matched route needs (see `crate::access`).
pub async fn owned_shop(state: &AppState, shop_id: Uuid, user: &AuthUser) -> AppResult<Shop> {
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1")
        .bind(shop_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if shop.owner_id == user.id || user.is_admin() {
        return Ok(shop);
    }
    let Some(need) = crate::access::current() else {
        return Err(AppError::Forbidden);
    };
    let ok: Option<bool> = sqlx::query_scalar(
        "UPDATE shop_members m SET last_seen_at = now()
         FROM shop_roles r WHERE r.id = m.role_id AND m.shop_id = $1 AND m.user_id = $2 AND m.active
         RETURNING $3 = ANY(r.permissions)",
    )
    .bind(shop_id)
    .bind(user.id)
    .bind(need)
    .fetch_optional(&state.db)
    .await?;
    match ok {
        Some(true) => Ok(shop),
        _ => Err(AppError::Forbidden),
    }
}

/// Load a shop the caller owns (admins bypass). Staff never pass, whatever their role.
pub async fn owner_shop(state: &AppState, shop_id: Uuid, user: &AuthUser) -> AppResult<Shop> {
    let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1")
        .bind(shop_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    if shop.owner_id != user.id && !user.is_admin() {
        return Err(AppError::Forbidden);
    }
    Ok(shop)
}

/// Load a shop by its public slug.
pub async fn public_shop(state: &AppState, slug: &str) -> AppResult<Shop> {
    sqlx::query_as("SELECT * FROM shops WHERE slug = $1")
        .bind(slug)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)
}
