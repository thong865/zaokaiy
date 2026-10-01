use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    ai,
    auth::AuthUser,
    error::{AppError, AppResult},
    models::Product,
    routes::{content_insert, owned_shop, CreateContentReq, CONTENT_KINDS},
    AppState,
};

pub async fn status(State(st): State<AppState>) -> Json<Value> {
    Json(json!({ "enabled": ai::enabled(&st), "provider": "openrouter", "model": st.cfg.ai_model }))
}

#[derive(Deserialize)]
pub struct GenerateReq {
    pub product_id: Uuid,
    pub kind: String,
    pub tone: Option<String>,
    pub language: Option<String>,
    pub extra: Option<String>,
    /// Save the result as a draft content item.
    pub save: Option<bool>,
}

pub async fn generate(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<GenerateReq>,
) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    if !CONTENT_KINDS.contains(&req.kind.as_str()) {
        return Err(AppError::bad(format!(
            "kind must be one of {CONTENT_KINDS:?}"
        )));
    }
    // The shop may write about its own products or products it resells.
    let product: Product = sqlx::query_as(
        "SELECT p.* FROM products p WHERE p.id = $1 AND (p.shop_id = $2
            OR EXISTS (SELECT 1 FROM listings l WHERE l.product_id = p.id AND l.reseller_shop_id = $2))",
    )
    .bind(req.product_id)
    .bind(shop_id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let out = ai::generate(
        &st,
        ai::GenerateInput {
            product: &product,
            shop: &shop,
            kind: &req.kind,
            tone: req.tone.as_deref().unwrap_or("friendly and energetic"),
            language: req.language.as_deref().unwrap_or("English"),
            extra: req.extra.as_deref().unwrap_or(""),
        },
    )
    .await?;

    let saved = if req.save.unwrap_or(false) {
        Some(
            content_insert(
                &st,
                shop_id,
                &CreateContentReq {
                    product_id: Some(product.id),
                    kind: req.kind.clone(),
                    title: out.title.clone(),
                    body: out.body.clone(),
                    media_url: None,
                    ai_generated: Some(out.ai),
                    status: Some("draft".into()),
                },
            )
            .await?,
        )
    } else {
        None
    };

    Ok(Json(
        json!({ "title": out.title, "body": out.body, "ai": out.ai, "saved": saved }),
    ))
}

#[derive(Deserialize)]
pub struct ChatMsg {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct AgentReq {
    pub messages: Vec<ChatMsg>,
}

pub async fn agent(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(req): Json<AgentReq>,
) -> AppResult<Json<Value>> {
    let shop = owned_shop(&st, shop_id, &user).await?;
    let history: Vec<Value> = req
        .messages
        .into_iter()
        .rev()
        .take(20)
        .rev()
        .filter(|m| (m.role == "user" || m.role == "assistant") && !m.content.trim().is_empty())
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    if history.last().map(|m| m["role"] != "user").unwrap_or(true) {
        return Err(AppError::bad("last message must be from the user"));
    }
    let r = ai::agent(&st, &shop, history).await?;
    Ok(Json(
        json!({ "reply": r.reply, "actions": r.actions, "ai": r.ai }),
    ))
}
