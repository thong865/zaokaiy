use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{auth::AuthUser, error::AppResult, routes::owned_shop, AppState};

pub async fn compute(st: &AppState, shop_id: Uuid) -> AppResult<Value> {
    let (revenue, orders, units): (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(oi.unit_price_cents * oi.qty),0)::bigint,
                COUNT(DISTINCT o.id), COALESCE(SUM(oi.qty),0)::bigint
         FROM order_items oi JOIN orders o ON o.id = oi.order_id
         WHERE oi.supplier_shop_id = $1 AND o.status IN ('paid','shipped','completed')",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;

    let (pending_orders,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT o.id) FROM orders o JOIN order_items oi ON oi.order_id=o.id
         WHERE oi.supplier_shop_id=$1 AND o.status='paid'",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;

    let (products, active, low_stock, in_review, rejected): (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE status='active' AND review_status='approved'),
                COUNT(*) FILTER (WHERE status<>'archived' AND stock <= low_stock_threshold),
                COUNT(*) FILTER (WHERE review_status='pending'),
                COUNT(*) FILTER (WHERE review_status='rejected' AND status<>'archived')
         FROM products WHERE shop_id=$1",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;

    let (earned, owed): (i64, i64) = sqlx::query_as(
        "SELECT
            COALESCE(SUM(amount_cents) FILTER (WHERE beneficiary_shop_id=$1 AND status IN ('pending','approved','paid')),0)::bigint,
            COALESCE(SUM(amount_cents) FILTER (WHERE supplier_shop_id=$1 AND status IN ('pending','approved')),0)::bigint
         FROM commissions WHERE beneficiary_shop_id=$1 OR supplier_shop_id=$1",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;

    let (ad_spend, ad_clicks, ad_impr): (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(spent_cents),0)::bigint, COALESCE(SUM(clicks),0)::bigint, COALESCE(SUM(impressions),0)::bigint
         FROM ad_campaigns WHERE shop_id=$1",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;

    let (resellers, pending_requests): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE status='approved'), COUNT(*) FILTER (WHERE status='pending')
         FROM partnerships WHERE supplier_shop_id=$1",
    )
    .bind(shop_id)
    .fetch_one(&st.db)
    .await?;

    let series: Vec<(chrono::NaiveDate, i64)> = sqlx::query_as(
        "SELECT d::date, COALESCE(SUM(oi.unit_price_cents * oi.qty),0)::bigint
         FROM generate_series(current_date - 13, current_date, interval '1 day') d
         LEFT JOIN orders o ON o.created_at::date = d::date AND o.status IN ('paid','shipped','completed')
         LEFT JOIN order_items oi ON oi.order_id = o.id AND oi.supplier_shop_id = $1
         GROUP BY d ORDER BY d",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;

    let top: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT oi.product_name, SUM(oi.qty)::bigint, SUM(oi.qty*oi.unit_price_cents)::bigint
         FROM order_items oi JOIN orders o ON o.id=oi.order_id
         WHERE oi.supplier_shop_id=$1 AND o.status IN ('paid','shipped','completed')
         GROUP BY oi.product_name ORDER BY 3 DESC LIMIT 5",
    )
    .bind(shop_id)
    .fetch_all(&st.db)
    .await?;

    Ok(json!({
        "revenue_cents": revenue, "orders": orders, "units_sold": units, "orders_to_ship": pending_orders,
        "products": products, "active_products": active, "in_review": in_review, "rejected": rejected, "low_stock": low_stock,
        "commission_earned_cents": earned, "commission_owed_cents": owed,
        "ad_spend_cents": ad_spend, "ad_clicks": ad_clicks, "ad_impressions": ad_impr,
        "resellers": resellers, "pending_reseller_requests": pending_requests,
        "revenue_series": series.into_iter().map(|(d, v)| json!({ "date": d, "revenue_cents": v })).collect::<Vec<_>>(),
        "top_products": top.into_iter().map(|(n, q, r)| json!({ "name": n, "units": q, "revenue_cents": r })).collect::<Vec<_>>()
    }))
}

pub async fn shop_stats(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, id, &user).await?;
    Ok(Json(compute(&st, id).await?))
}
