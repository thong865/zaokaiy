//! Terminal authentication: `Authorization: Bearer zkd_<token>`.

use axum::{extract::FromRequestParts, http::request::Parts};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::Shop,
    AppState,
};

use super::sha256_hex;

pub const TOKEN_PREFIX: &str = "zkd_";

/// A paired, not revoked terminal whose user still has POS access to its shop.
pub struct Device {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub shop: Shop,
    pub user: AuthUser,
}

impl FromRequestParts<AppState> for Device {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, st: &AppState) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| t.starts_with(TOKEN_PREFIX) && t.len() < 200)
            .ok_or(AppError::Unauthorized)?;
        let row: Option<(Uuid, Uuid, Uuid, String, String, String)> = sqlx::query_as(
            "UPDATE pos_devices d SET last_seen_at = now() FROM users u
             WHERE d.token_hash = $1 AND d.revoked_at IS NULL AND u.id = d.user_id
             RETURNING d.id, d.shop_id, d.user_id, d.code, d.name, u.role",
        )
        .bind(sha256_hex(token))
        .fetch_optional(&st.db)
        .await?;
        let (id, shop_id, user_id, code, name, role) = row.ok_or(AppError::Unauthorized)?;
        let shop: Shop = sqlx::query_as("SELECT * FROM shops WHERE id = $1").bind(shop_id).fetch_one(&st.db).await?;
        let user = AuthUser { id: user_id, role };
        if !has_perm(st, &shop, &user, "pos").await? {
            return Err(AppError::Forbidden);
        }
        Ok(Device { id, code, name, shop, user })
    }
}

/// Owner, platform admin, or an active staff member whose role has `perm`.
pub async fn has_perm(st: &AppState, shop: &Shop, user: &AuthUser, perm: &str) -> AppResult<bool> {
    if shop.owner_id == user.id || user.is_admin() {
        return Ok(true);
    }
    let ok: Option<bool> = sqlx::query_scalar(
        "SELECT $3 = ANY(r.permissions) FROM shop_members m JOIN shop_roles r ON r.id = m.role_id
         WHERE m.shop_id = $1 AND m.user_id = $2 AND m.active",
    )
    .bind(shop.id)
    .bind(user.id)
    .bind(perm)
    .fetch_optional(&st.db)
    .await?;
    Ok(ok == Some(true))
}
