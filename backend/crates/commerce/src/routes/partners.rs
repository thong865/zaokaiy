//! Reseller ("sell staff") program: partnerships, listings and commissions.

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
    models::{Commission, Partnership},
    routes::owned_shop,
    AppState,
};

const PARTNERSHIP_SELECT: &str = "SELECT pa.id, pa.supplier_shop_id, su.name AS supplier_name,
        pa.reseller_shop_id, re.name AS reseller_name, pa.status, pa.commission_bps, pa.message,
        pa.created_at, pa.decided_at
     FROM partnerships pa
     JOIN shops su ON su.id = pa.supplier_shop_id
     JOIN shops re ON re.id = pa.reseller_shop_id";

#[derive(Deserialize)]
pub struct MarketQ {
    /// The caller's reseller shop, used to annotate partnership/listing status.
    pub shop_id: Option<Uuid>,
    pub q: Option<String>,
    pub category: Option<String>,
}

pub async fn marketplace(
    State(st): State<AppState>,
    user: AuthUser,
    Query(q): Query<MarketQ>,
) -> AppResult<Json<Vec<Value>>> {
    if let Some(sid) = q.shop_id {
        owned_shop(&st, sid, &user).await?;
    }
    let rows: Vec<(Uuid, Uuid, String, String, i64, serde_json::Value, String, i32, i32, String, String, Option<String>, Option<i32>, bool, Option<serde_json::Value>)> =
        sqlx::query_as(
            "SELECT p.id, p.shop_id, p.name, p.description, p.price_cents, p.images, p.category, p.stock,
                    p.commission_bps, s.name, s.slug, pa.status, pa.commission_bps,
                    EXISTS (SELECT 1 FROM listings l WHERE l.product_id = p.id AND l.reseller_shop_id = $1 AND l.active), p.cover
             FROM products p
             JOIN shops s ON s.id = p.shop_id
             LEFT JOIN partnerships pa ON pa.supplier_shop_id = p.shop_id AND pa.reseller_shop_id = $1
             WHERE p.allow_resell AND p.status = 'active' AND p.review_status = 'approved' AND s.owner_id <> $2
               AND ($3::text IS NULL OR p.name ILIKE '%' || $3 || '%')
               AND ($4::text IS NULL OR p.category = $4)
             ORDER BY p.commission_bps DESC, p.created_at DESC LIMIT 100",
        )
        .bind(q.shop_id)
        .bind(user.id)
        .bind(q.q.filter(|s| !s.trim().is_empty()))
        .bind(q.category.filter(|s| !s.trim().is_empty()))
        .fetch_all(&st.db)
        .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    shop_id,
                    name,
                    desc,
                    price,
                    images,
                    cat,
                    stock,
                    bps,
                    shop_name,
                    shop_slug,
                    pstatus,
                    pbps,
                    listed,
                    cover,
                )| {
                    let effective = pbps.unwrap_or(bps);
                    json!({
                        "id": id, "shop_id": shop_id, "name": name, "description": desc,
                        "price_cents": price, "images": images, "category": cat, "stock": stock,
                        "commission_bps": bps, "effective_commission_bps": effective,
                        "commission_per_unit_cents": price * effective as i64 / 10_000,
                        "shop_name": shop_name, "shop_slug": shop_slug,
                        "partnership_status": pstatus, "listed": listed, "cover": cover
                    })
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct RequestReq {
    pub supplier_shop_id: Uuid,
    pub message: Option<String>,
}

/// Reseller shop `id` asks to become a sell-staff of `supplier_shop_id`.
pub async fn request(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<RequestReq>,
) -> AppResult<Json<Partnership>> {
    owned_shop(&st, id, &user).await?;
    if req.supplier_shop_id == id {
        return Err(AppError::bad("cannot partner with your own shop"));
    }
    let pid: Uuid = sqlx::query_scalar(
        "INSERT INTO partnerships (supplier_shop_id, reseller_shop_id, message)
         VALUES ($1,$2,$3)
         ON CONFLICT (supplier_shop_id, reseller_shop_id) DO UPDATE
           SET status = CASE WHEN partnerships.status IN ('rejected','revoked') THEN 'pending' ELSE partnerships.status END,
               message = EXCLUDED.message
         RETURNING id",
    )
    .bind(req.supplier_shop_id)
    .bind(id)
    .bind(req.message.unwrap_or_default())
    .fetch_one(&st.db)
    .await?;
    fetch_partnership(&st, pid).await.map(Json)
}

async fn fetch_partnership(st: &AppState, id: Uuid) -> AppResult<Partnership> {
    Ok(
        sqlx::query_as(&format!("{PARTNERSHIP_SELECT} WHERE pa.id = $1"))
            .bind(id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?,
    )
}

#[derive(Deserialize)]
pub struct ListQ {
    /// "supplier" = requests to me, "reseller" = my requests. Default both.
    #[serde(rename = "as")]
    pub as_: Option<String>,
}

pub async fn list(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<ListQ>,
) -> AppResult<Json<Vec<Partnership>>> {
    owned_shop(&st, id, &user).await?;
    let filter = match q.as_.as_deref() {
        Some("supplier") => "pa.supplier_shop_id = $1",
        Some("reseller") => "pa.reseller_shop_id = $1",
        _ => "(pa.supplier_shop_id = $1 OR pa.reseller_shop_id = $1)",
    };
    let rows = sqlx::query_as(&format!(
        "{PARTNERSHIP_SELECT} WHERE {filter} ORDER BY pa.created_at DESC"
    ))
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct DecideReq {
    pub approve: bool,
    /// Optional override of product commission for this reseller (bps).
    pub commission_bps: Option<i32>,
}

pub async fn decide(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<DecideReq>,
) -> AppResult<Json<Partnership>> {
    let p = fetch_partnership(&st, id).await?;
    owned_shop(&st, p.supplier_shop_id, &user).await?;
    if let Some(b) = req.commission_bps {
        if !(0..=9000).contains(&b) {
            return Err(AppError::bad("commission_bps must be 0..9000"));
        }
    }
    sqlx::query(
        "UPDATE partnerships SET status = $2, commission_bps = COALESCE($3, commission_bps), decided_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(if req.approve { "approved" } else { "rejected" })
    .bind(req.commission_bps)
    .execute(&st.db)
    .await?;
    fetch_partnership(&st, id).await.map(Json)
}

pub async fn revoke(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Partnership>> {
    let p = fetch_partnership(&st, id).await?;
    let is_supplier = owned_shop(&st, p.supplier_shop_id, &user).await.is_ok();
    let is_reseller = owned_shop(&st, p.reseller_shop_id, &user).await.is_ok();
    if !is_supplier && !is_reseller {
        return Err(AppError::Forbidden);
    }
    sqlx::query("UPDATE partnerships SET status='revoked', decided_at=now() WHERE id=$1")
        .bind(id)
        .execute(&st.db)
        .await?;
    fetch_partnership(&st, id).await.map(Json)
}

pub async fn listings(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, id, &user).await?;
    let rows: Vec<(Uuid, Uuid, String, i64, serde_json::Value, i32, String, i32, Option<String>, bool, Option<serde_json::Value>)> = sqlx::query_as(
        "SELECT l.id, p.id, p.name, p.price_cents, p.images, p.stock, s.name,
                COALESCE(pa.commission_bps, p.commission_bps), pa.status,
                (l.active AND p.status = 'active' AND p.review_status = 'approved' AND p.allow_resell AND pa.status='approved'), p.cover
         FROM listings l
         JOIN products p ON p.id = l.product_id
         JOIN shops s ON s.id = p.shop_id
         LEFT JOIN partnerships pa ON pa.supplier_shop_id = p.shop_id AND pa.reseller_shop_id = l.reseller_shop_id
         WHERE l.reseller_shop_id = $1 ORDER BY l.created_at DESC",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(lid, pid, name, price, images, stock, supplier, bps, pstatus, live, cover)| {
                json!({
                    "id": lid, "product_id": pid, "name": name, "price_cents": price, "images": images,
                    "stock": stock, "supplier_name": supplier, "commission_bps": bps,
                    "commission_per_unit_cents": price * bps as i64 / 10_000,
                    "partnership_status": pstatus, "live": live, "cover": cover
                })
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct AddListing {
    pub product_id: Uuid,
}

pub async fn add_listing(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<AddListing>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, id, &user).await?;
    let ok: Option<bool> = sqlx::query_scalar(
        "SELECT (p.allow_resell AND p.status = 'active' AND p.review_status = 'approved' AND EXISTS (
                    SELECT 1 FROM partnerships pa
                    WHERE pa.supplier_shop_id = p.shop_id AND pa.reseller_shop_id = $2 AND pa.status='approved'))
         FROM products p WHERE p.id = $1",
    )
    .bind(req.product_id)
    .bind(id)
    .fetch_optional(&st.db)
    .await?;
    match ok {
        None => return Err(AppError::NotFound),
        Some(false) => return Err(AppError::bad(
            "product is not open for resale or your partnership with the supplier is not approved",
        )),
        Some(true) => {}
    }
    let lid: Uuid = sqlx::query_scalar(
        "INSERT INTO listings (reseller_shop_id, product_id) VALUES ($1,$2)
         ON CONFLICT (reseller_shop_id, product_id) DO UPDATE SET active = true RETURNING id",
    )
    .bind(id)
    .bind(req.product_id)
    .fetch_one(&st.db)
    .await?;
    Ok(Json(json!({ "id": lid })))
}

pub async fn remove_listing(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let shop_id: Uuid = sqlx::query_scalar("SELECT reseller_shop_id FROM listings WHERE id=$1")
        .bind(id)
        .fetch_optional(&st.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(&st, shop_id, &user).await?;
    sqlx::query("DELETE FROM listings WHERE id=$1")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

const COMMISSION_SELECT: &str =
    "SELECT c.id, c.order_item_id, oi.order_id, oi.product_name, c.beneficiary_shop_id,
        c.supplier_shop_id, s.name AS supplier_name, c.amount_cents, c.status, c.created_at
     FROM commissions c
     JOIN order_items oi ON oi.id = c.order_item_id
     JOIN shops s ON s.id = c.supplier_shop_id";

async fn commission_summary(st: &AppState, col: &str, shop_id: Uuid) -> AppResult<Value> {
    let rows: Vec<(String, i64)> = sqlx::query_as(&format!(
        "SELECT status, COALESCE(SUM(amount_cents),0)::bigint FROM commissions WHERE {col} = $1 GROUP BY status"
    ))
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;
    let mut m = serde_json::Map::new();
    for s in ["pending", "approved", "paid", "void"] {
        m.insert(s.into(), json!(0));
    }
    for (s, v) in rows {
        m.insert(s, json!(v));
    }
    Ok(Value::Object(m))
}

/// Commissions this shop has earned as a reseller.
pub async fn commissions_earned(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, id, &user).await?;
    let items: Vec<Commission> = sqlx::query_as(&format!(
        "{COMMISSION_SELECT} WHERE c.beneficiary_shop_id = $1 ORDER BY c.created_at DESC LIMIT 200"
    ))
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    let summary = commission_summary(&st, "beneficiary_shop_id", id).await?;
    Ok(Json(json!({ "summary": summary, "items": items })))
}

/// Commissions this shop owes to its sell-staff.
pub async fn commissions_payable(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, id, &user).await?;
    let items: Vec<(
        Uuid,
        Uuid,
        String,
        String,
        i64,
        String,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT c.id, oi.order_id, oi.product_name, b.name, c.amount_cents, c.status, c.created_at
         FROM commissions c
         JOIN order_items oi ON oi.id = c.order_item_id
         JOIN shops b ON b.id = c.beneficiary_shop_id
         WHERE c.supplier_shop_id = $1 ORDER BY c.created_at DESC LIMIT 200",
    )
    .bind(id)
    .fetch_all(&st.db)
    .await?;
    let summary = commission_summary(&st, "supplier_shop_id", id).await?;
    let items: Vec<Value> = items
        .into_iter()
        .map(|(cid, oid, pname, bname, amt, status, at)| {
            json!({ "id": cid, "order_id": oid, "product_name": pname, "reseller_name": bname,
                    "amount_cents": amt, "status": status, "created_at": at })
        })
        .collect();
    Ok(Json(json!({ "summary": summary, "items": items })))
}

pub async fn pay_commission(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let (supplier, status): (Uuid, String) =
        sqlx::query_as("SELECT supplier_shop_id, status FROM commissions WHERE id=$1")
            .bind(id)
            .fetch_optional(&st.db)
            .await?
            .ok_or(AppError::NotFound)?;
    owned_shop(&st, supplier, &user).await?;
    if status != "approved" {
        return Err(AppError::bad(
            "only approved commissions (order completed) can be marked paid",
        ));
    }
    sqlx::query("UPDATE commissions SET status='paid', updated_at=now() WHERE id=$1")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
