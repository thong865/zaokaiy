use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    auth::{hash_password, issue_token, verify_password, AuthUser},
    error::{AppError, AppResult},
    models::User,
    AppState,
};

#[derive(Deserialize)]
pub struct RegisterReq {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

pub async fn register(
    State(st): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> AppResult<Json<Value>> {
    let email = req.email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 5 {
        return Err(AppError::bad("invalid email"));
    }
    if req.password.len() < 8 {
        return Err(AppError::bad("password must be at least 8 characters"));
    }
    let name = req.display_name.trim();
    if name.is_empty() {
        return Err(AppError::bad("display_name is required"));
    }
    let hash = hash_password(&req.password)?;
    let user: User = sqlx::query_as(
        "INSERT INTO users (email, password_hash, display_name, role) VALUES ($1,$2,$3,$4) RETURNING *",
    )
    .bind(&email)
    .bind(&hash)
    .bind(name)
    .bind(if st.cfg.admin_emails.contains(&email) { "admin" } else { "user" })
    .fetch_one(&st.db)
    .await
    .map_err(|e| match AppError::from(e) {
        AppError::Conflict(_) => AppError::Conflict("email already registered".into()),
        other => other,
    })?;
    let token = issue_token(&st, user.id, &user.role)?;
    Ok(Json(json!({ "token": token, "user": user })))
}

pub async fn login(
    State(st): State<AppState>,
    Json(req): Json<LoginReq>,
) -> AppResult<Json<Value>> {
    let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE email = $1")
        .bind(req.email.trim().to_lowercase())
        .fetch_optional(&st.db)
        .await?;
    let user = user
        .filter(|u| verify_password(&req.password, &u.password_hash))
        .ok_or_else(|| AppError::bad("invalid email or password"))?;
    let token = issue_token(&st, user.id, &user.role)?;
    Ok(Json(json!({ "token": token, "user": user })))
}

pub async fn me(State(st): State<AppState>, user: AuthUser) -> AppResult<Json<User>> {
    let u: User = sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(user.id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(Json(u))
}
