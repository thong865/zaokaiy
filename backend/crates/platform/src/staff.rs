//! Shop staff: roles (named permission sets), one-time invite links, and members.
//!
//! Only the shop owner (or a platform admin) manages staff. A person joins by opening an invite
//! link while signed in; they then see the shop in their dashboard with just the functions their
//! role allows (enforced per API route in `crate::access`).

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection};
use uuid::Uuid;

use crate::{
    access::{PERMISSIONS, TEMPLATES},
    auth::AuthUser,
    error::{AppError, AppResult},
    routes::owner_shop,
    AppState,
};

#[derive(Debug, Serialize, FromRow)]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    pub permissions: Vec<String>,
    pub template: Option<String>,
    pub members: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Member {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub role_id: Uuid,
    pub role: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Invite {
    pub id: Uuid,
    pub role_id: Uuid,
    pub role: String,
    pub note: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_name: Option<String>,
    pub revoked_at: Option<DateTime<Utc>>,
}

fn token_hash(t: &str) -> String {
    hex::encode(Sha256::digest(format!("zaokaiy-invite|{}", t.trim())))
}

fn clean_perms(p: Vec<String>) -> AppResult<Vec<String>> {
    if !crate::access::valid(&p) {
        return Err(AppError::bad("unknown permission"));
    }
    // Keep the canonical order, no duplicates.
    Ok(PERMISSIONS.iter().filter(|x| p.iter().any(|y| y == *x)).map(|x| x.to_string()).collect())
}

fn clean_name(n: &str) -> AppResult<String> {
    let n: String = n.trim().chars().take(60).collect();
    if n.is_empty() {
        return Err(AppError::bad("give the role a name"));
    }
    Ok(n)
}

/// Starter roles, created once per shop, named in the owner's language (English or Lao).
async fn ensure_templates(conn: &mut PgConnection, shop_id: Uuid, lao: bool) -> AppResult<()> {
    let any: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM shop_roles WHERE shop_id = $1)").bind(shop_id).fetch_one(&mut *conn).await?;
    if any {
        return Ok(());
    }
    for (key, name, perms) in TEMPLATES {
        let name = if lao { crate::access::TEMPLATE_NAMES_LO.iter().find(|(k, _)| *k == key).map(|(_, n)| *n).unwrap_or(name) } else { name };
        sqlx::query("INSERT INTO shop_roles (shop_id, name, permissions, template) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
            .bind(shop_id)
            .bind(name)
            .bind(perms.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            .bind(key)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

async fn doc(st: &AppState, shop_id: Uuid) -> AppResult<Value> {
    let roles: Vec<Role> = sqlx::query_as(
        "SELECT r.id, r.name, r.permissions, r.template, (SELECT count(*) FROM shop_members m WHERE m.role_id = r.id) AS members
         FROM shop_roles r WHERE r.shop_id = $1 ORDER BY r.created_at, r.name",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    let members: Vec<Member> = sqlx::query_as(
        "SELECT m.user_id, u.display_name, u.email, u.phone, m.role_id, r.name AS role, m.active, m.created_at, m.last_seen_at
         FROM shop_members m JOIN users u ON u.id = m.user_id JOIN shop_roles r ON r.id = m.role_id
         WHERE m.shop_id = $1 ORDER BY m.active DESC, u.display_name",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    let invites: Vec<Invite> = sqlx::query_as(
        "SELECT i.id, i.role_id, r.name AS role, i.note, i.created_at, i.expires_at, i.accepted_at, u.display_name AS accepted_name, i.revoked_at
         FROM shop_invites i JOIN shop_roles r ON r.id = i.role_id LEFT JOIN users u ON u.id = i.accepted_by
         WHERE i.shop_id = $1 AND i.created_at > now() - interval '30 days' ORDER BY i.created_at DESC LIMIT 50",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    Ok(json!({ "roles": roles, "members": members, "invites": invites, "permissions": PERMISSIONS }))
}

pub async fn overview(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, headers: axum::http::HeaderMap) -> AppResult<Json<Value>> {
    owner_shop(&st, shop_id, &user).await?;
    let lao = headers
        .get(axum::http::header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim().to_ascii_lowercase().starts_with("lo"));
    let mut conn = st.db.acquire().await?;
    ensure_templates(&mut conn, shop_id, lao).await?;
    drop(conn);
    Ok(Json(doc(&st, shop_id).await?))
}

#[derive(Deserialize)]
pub struct RoleReq {
    pub name: Option<String>,
    pub permissions: Option<Vec<String>>,
}

pub async fn create_role(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<RoleReq>) -> AppResult<Json<Value>> {
    owner_shop(&st, shop_id, &user).await?;
    let name = clean_name(r.name.as_deref().unwrap_or_default())?;
    let perms = clean_perms(r.permissions.unwrap_or_default())?;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM shop_roles WHERE shop_id = $1").bind(shop_id).fetch_one(&st.db).await?;
    if n >= 30 {
        return Err(AppError::bad("a shop can have at most 30 roles"));
    }
    sqlx::query("INSERT INTO shop_roles (shop_id, name, permissions) VALUES ($1,$2,$3)")
        .bind(shop_id)
        .bind(&name)
        .bind(&perms)
        .execute(&st.db)
        .await
        .map_err(|e| match AppError::from(e) {
            AppError::Conflict(_) => AppError::Conflict("a role with this name already exists".into()),
            o => o,
        })?;
    Ok(Json(doc(&st, shop_id).await?))
}

async fn role_shop(st: &AppState, id: Uuid) -> AppResult<Uuid> {
    sqlx::query_scalar("SELECT shop_id FROM shop_roles WHERE id = $1").bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)
}

pub async fn update_role(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>, Json(r): Json<RoleReq>) -> AppResult<Json<Value>> {
    let shop_id = role_shop(&st, id).await?;
    owner_shop(&st, shop_id, &user).await?;
    let name = r.name.as_deref().map(clean_name).transpose()?;
    let perms = r.permissions.map(clean_perms).transpose()?;
    sqlx::query("UPDATE shop_roles SET name = COALESCE($2, name), permissions = COALESCE($3, permissions), updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(name)
        .bind(perms)
        .execute(&st.db)
        .await
        .map_err(|e| match AppError::from(e) {
            AppError::Conflict(_) => AppError::Conflict("a role with this name already exists".into()),
            o => o,
        })?;
    Ok(Json(doc(&st, shop_id).await?))
}

pub async fn delete_role(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let shop_id = role_shop(&st, id).await?;
    owner_shop(&st, shop_id, &user).await?;
    let used: i64 = sqlx::query_scalar("SELECT count(*) FROM shop_members WHERE role_id = $1").bind(id).fetch_one(&st.db).await?;
    if used > 0 {
        return Err(AppError::Conflict(format!("{used} staff member(s) have this role — give them another role first")));
    }
    sqlx::query("DELETE FROM shop_roles WHERE id = $1").bind(id).execute(&st.db).await?;
    Ok(Json(doc(&st, shop_id).await?))
}

#[derive(Deserialize)]
pub struct InviteReq {
    pub role_id: Uuid,
    #[serde(default)]
    pub note: String,
    /// Link lifetime (1–30 days, default 7).
    pub days: Option<i64>,
}

/// Create a one-time invite link. The token is returned once; only its hash is stored.
pub async fn create_invite(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>, Json(r): Json<InviteReq>) -> AppResult<Json<Value>> {
    owner_shop(&st, shop_id, &user).await?;
    if role_shop(&st, r.role_id).await? != shop_id {
        return Err(AppError::bad("choose one of this shop's roles"));
    }
    let open: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_invites WHERE shop_id = $1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;
    if open >= 50 {
        return Err(AppError::bad("too many open invites — revoke unused links first"));
    }
    let mut raw = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut raw);
    let token = hex::encode(raw);
    let days = r.days.unwrap_or(7).clamp(1, 30);
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO shop_invites (shop_id, role_id, token_hash, note, created_by, expires_at) VALUES ($1,$2,$3,$4,$5,$6) RETURNING id",
    )
    .bind(shop_id)
    .bind(r.role_id)
    .bind(token_hash(&token))
    .bind(r.note.trim().chars().take(120).collect::<String>())
    .bind(user.id)
    .bind(Utc::now() + Duration::days(days))
    .fetch_one(&st.db)
    .await?;
    let link = format!("{}/join/{token}", st.cfg.public_web_url.trim_end_matches('/'));
    Ok(Json(json!({ "id": id, "token": token, "link": link, "staff": doc(&st, shop_id).await? })))
}

pub async fn revoke_invite(State(st): State<AppState>, user: AuthUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let shop_id: Uuid = sqlx::query_scalar("SELECT shop_id FROM shop_invites WHERE id = $1").bind(id).fetch_optional(&st.db).await?.ok_or(AppError::NotFound)?;
    owner_shop(&st, shop_id, &user).await?;
    sqlx::query("UPDATE shop_invites SET revoked_at = now() WHERE id = $1 AND accepted_at IS NULL AND revoked_at IS NULL").bind(id).execute(&st.db).await?;
    Ok(Json(doc(&st, shop_id).await?))
}

#[derive(Deserialize)]
pub struct MemberReq {
    pub role_id: Option<Uuid>,
    pub active: Option<bool>,
}

pub async fn update_member(State(st): State<AppState>, user: AuthUser, Path((shop_id, uid)): Path<(Uuid, Uuid)>, Json(r): Json<MemberReq>) -> AppResult<Json<Value>> {
    owner_shop(&st, shop_id, &user).await?;
    if let Some(rid) = r.role_id {
        if role_shop(&st, rid).await? != shop_id {
            return Err(AppError::bad("choose one of this shop's roles"));
        }
    }
    let n = sqlx::query("UPDATE shop_members SET role_id = COALESCE($3, role_id), active = COALESCE($4, active) WHERE shop_id = $1 AND user_id = $2")
        .bind(shop_id)
        .bind(uid)
        .bind(r.role_id)
        .bind(r.active)
        .execute(&st.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(doc(&st, shop_id).await?))
}

pub async fn remove_member(State(st): State<AppState>, user: AuthUser, Path((shop_id, uid)): Path<(Uuid, Uuid)>) -> AppResult<Json<Value>> {
    owner_shop(&st, shop_id, &user).await?;
    let n = sqlx::query("DELETE FROM shop_members WHERE shop_id = $1 AND user_id = $2").bind(shop_id).bind(uid).execute(&st.db).await?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(doc(&st, shop_id).await?))
}

#[derive(FromRow)]
struct InviteRow {
    id: Uuid,
    shop_id: Uuid,
    shop_name: String,
    shop_slug: String,
    logo_url: Option<String>,
    owner_id: Uuid,
    role_id: Uuid,
    role: String,
    permissions: Vec<String>,
    expires_at: DateTime<Utc>,
    accepted_at: Option<DateTime<Utc>>,
    accepted_by: Option<Uuid>,
    revoked_at: Option<DateTime<Utc>>,
}

async fn load_invite(conn: &mut PgConnection, token: &str, lock: bool) -> AppResult<InviteRow> {
    sqlx::query_as(&format!(
        "SELECT i.id, i.shop_id, s.name AS shop_name, s.slug AS shop_slug, s.logo_url, s.owner_id, i.role_id, r.name AS role, r.permissions,
                i.expires_at, i.accepted_at, i.accepted_by, i.revoked_at
         FROM shop_invites i JOIN shops s ON s.id = i.shop_id JOIN shop_roles r ON r.id = i.role_id
         WHERE i.token_hash = $1 {}",
        if lock { "FOR UPDATE OF i" } else { "" }
    ))
    .bind(token_hash(token))
    .fetch_optional(conn)
    .await?
    .ok_or(AppError::NotFound)
}

fn invite_state(i: &InviteRow) -> &'static str {
    if i.revoked_at.is_some() {
        "revoked"
    } else if i.accepted_at.is_some() {
        "used"
    } else if i.expires_at <= Utc::now() {
        "expired"
    } else {
        "open"
    }
}

/// What an invite link is for (shown before accepting; no sign-in needed).
pub async fn invite_info(State(st): State<AppState>, Path(token): Path<String>) -> AppResult<Json<Value>> {
    let mut conn = st.db.acquire().await?;
    let i = load_invite(&mut conn, &token, false).await?;
    Ok(Json(json!({
        "shop": { "id": i.shop_id, "name": i.shop_name, "slug": i.shop_slug, "logo_url": i.logo_url },
        "role": i.role, "permissions": i.permissions, "expires_at": i.expires_at, "state": invite_state(&i),
    })))
}

/// Join the shop with the invite's role (signed in). One use per link.
pub async fn accept_invite(State(st): State<AppState>, user: AuthUser, Path(token): Path<String>) -> AppResult<Json<Value>> {
    let mut tx = st.db.begin().await?;
    let i = load_invite(&mut tx, &token, true).await?;
    match invite_state(&i) {
        "open" => {}
        "used" if i.accepted_by == Some(user.id) => return Ok(Json(json!({ "shop_id": i.shop_id, "joined": true }))),
        "expired" => return Err(AppError::bad("this invite link has expired — ask the shop for a new one")),
        _ => return Err(AppError::bad("this invite link is no longer valid — ask the shop for a new one")),
    }
    if i.owner_id == user.id {
        return Err(AppError::bad("you own this shop"));
    }
    sqlx::query(
        "INSERT INTO shop_members (shop_id, user_id, role_id, invited_by)
         VALUES ($1, $2, $3, (SELECT created_by FROM shop_invites WHERE id = $4))
         ON CONFLICT (shop_id, user_id) DO UPDATE SET role_id = EXCLUDED.role_id, active = true",
    )
    .bind(i.shop_id)
    .bind(user.id)
    .bind(i.role_id)
    .bind(i.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE shop_invites SET accepted_by = $2, accepted_at = now() WHERE id = $1").bind(i.id).bind(user.id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Json(json!({ "shop_id": i.shop_id, "joined": true, "role": i.role })))
}

/// A staff member leaves a shop.
pub async fn leave(State(st): State<AppState>, user: AuthUser, Path(shop_id): Path<Uuid>) -> AppResult<Json<Value>> {
    let n = sqlx::query("DELETE FROM shop_members WHERE shop_id = $1 AND user_id = $2").bind(shop_id).bind(user.id).execute(&st.db).await?.rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "left": shop_id })))
}
