use std::collections::BTreeMap;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{Order, OrderItem},
    routes::{catalog::RESELL_OK, commission_of, logistics, owned_shop},
    AppState,
};

#[derive(Deserialize)]
pub struct CheckoutItem {
    pub product_id: Uuid,
    pub qty: i32,
    /// Reseller storefront the buyer came through (earns commission).
    pub via_shop_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct CheckoutReq {
    pub items: Vec<CheckoutItem>,
    pub shipping_address: Value,
    /// Delivery choice per supplier shop. Required for shops that have couriers set up.
    #[serde(default)]
    pub delivery: Vec<DeliveryChoice>,
}

#[derive(Deserialize)]
pub struct DeliveryChoice {
    pub shop_id: Uuid,
    pub carrier_code: String,
    /// branch (pick up at the courier's branch) | home
    #[serde(default = "branch")]
    pub delivery_type: String,
    /// prepaid | cod
    #[serde(default = "prepaid")]
    pub payment_method: String,
}
fn branch() -> String {
    "branch".into()
}
fn prepaid() -> String {
    "prepaid".into()
}

#[derive(sqlx::FromRow)]
struct LockedProduct {
    id: Uuid,
    shop_id: Uuid,
    name: String,
    price_cents: i64,
    status: String,
    review_status: String,
    stock: i32,
    currency: String,
}

async fn load_orders(st: &AppState, ids: &[Uuid]) -> AppResult<Vec<Value>> {
    let orders: Vec<Order> =
        sqlx::query_as("SELECT * FROM orders WHERE id = ANY($1) ORDER BY created_at DESC")
            .bind(ids)
            .fetch_all(&st.db)
            .await?;
    let items: Vec<OrderItem> =
        sqlx::query_as("SELECT * FROM order_items WHERE order_id = ANY($1)")
            .bind(ids)
            .fetch_all(&st.db)
            .await?;
    let mut by_order: BTreeMap<Uuid, Vec<&OrderItem>> = BTreeMap::new();
    for it in &items {
        by_order.entry(it.order_id).or_default().push(it);
    }
    Ok(orders
        .iter()
        .map(|o| {
            let mut v = serde_json::to_value(o).unwrap();
            v["items"] = json!(by_order.get(&o.id).cloned().unwrap_or_default());
            v
        })
        .collect())
}

/// Places one order per supplier shop. Stock is reserved atomically with row locks.
pub async fn checkout(
    State(st): State<AppState>,
    user: AuthUser,
    Json(req): Json<CheckoutReq>,
) -> AppResult<Json<Vec<Value>>> {
    if req.items.is_empty() {
        return Err(AppError::bad("cart is empty"));
    }
    // Merge duplicate lines.
    let mut lines: BTreeMap<(Uuid, Option<Uuid>), i32> = BTreeMap::new();
    for it in &req.items {
        if !(1..=100).contains(&it.qty) {
            return Err(AppError::bad("qty must be between 1 and 100"));
        }
        *lines.entry((it.product_id, it.via_shop_id)).or_default() += it.qty;
    }
    let product_ids: Vec<Uuid> = {
        let mut v: Vec<Uuid> = lines.keys().map(|(p, _)| *p).collect();
        v.sort();
        v.dedup();
        v
    };

    let mut tx = st.db.begin().await?;
    let locked: Vec<LockedProduct> = sqlx::query_as(
        "SELECT p.id, p.shop_id, p.name, p.price_cents, p.status, p.review_status, p.stock, s.currency
         FROM products p JOIN shops s ON s.id = p.shop_id
         WHERE p.id = ANY($1) ORDER BY p.id FOR UPDATE OF p",
    )
    .bind(&product_ids)
    .fetch_all(&mut *tx)
    .await?;
    let products: BTreeMap<Uuid, LockedProduct> = locked.into_iter().map(|p| (p.id, p)).collect();

    // Validate availability.
    let mut need: BTreeMap<Uuid, i32> = BTreeMap::new();
    for ((pid, _), qty) in &lines {
        *need.entry(*pid).or_default() += qty;
    }
    for (pid, qty) in &need {
        let p = products
            .get(pid)
            .ok_or_else(|| AppError::bad(format!("product {pid} not found")))?;
        if p.status != "active" || p.review_status != "approved" {
            return Err(AppError::bad(format!("'{}' is not available", p.name)));
        }
        if p.stock < *qty {
            return Err(AppError::bad(format!(
                "'{}' has only {} left",
                p.name, p.stock
            )));
        }
    }
    // Vehicles are sold online only when the seller allows it and the vehicle is still available.
    let offline: Option<String> = sqlx::query_scalar(
        "SELECT p.name FROM vehicle_specs v JOIN products p ON p.id = v.product_id
         WHERE v.product_id = ANY($1) AND (NOT v.buy_online OR v.sale_status <> 'available') LIMIT 1",
    )
    .bind(&product_ids)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(name) = offline {
        return Err(AppError::bad(format!("'{name}' is not available")));
    }

    // Resolve commission rate for each (product, via) line.
    struct Line<'a> {
        p: &'a LockedProduct,
        qty: i32,
        seller: Option<Uuid>,
        bps: i32,
    }
    let mut grouped: BTreeMap<Uuid, Vec<Line>> = BTreeMap::new();
    for ((pid, via), qty) in &lines {
        let p = &products[pid];
        let (seller, bps) = match via {
            Some(v) if *v != p.shop_id => {
                let sql = format!(
                    "SELECT {ok}, COALESCE((SELECT pa.commission_bps FROM partnerships pa
                         WHERE pa.supplier_shop_id = p.shop_id AND pa.reseller_shop_id = $2), p.commission_bps),
                         (SELECT owner_id FROM shops WHERE id = $2)
                     FROM products p WHERE p.id = $1",
                    ok = RESELL_OK.replace("$via", "$2")
                );
                let (ok, bps, owner): (bool, i32, Option<Uuid>) = sqlx::query_as(&sql)
                    .bind(pid)
                    .bind(v)
                    .fetch_one(&mut *tx)
                    .await?;
                if !ok {
                    return Err(AppError::bad(format!(
                        "'{}' is no longer sold by that shop",
                        p.name
                    )));
                }
                // No self-commission when the reseller buys through their own storefront.
                let bps = if owner == Some(user.id) { 0 } else { bps };
                (Some(*v), bps)
            }
            _ => (None, 0),
        };
        grouped.entry(p.shop_id).or_default().push(Line {
            p,
            qty: *qty,
            seller,
            bps,
        });
    }

    let mut order_ids = Vec::new();
    for (supplier, lines) in grouped {
        let total: i64 = lines.iter().map(|l| l.p.price_cents * l.qty as i64).sum();
        let currency = lines[0].p.currency.clone();
        let choice = req.delivery.iter().find(|d| d.shop_id == supplier);
        let q = match choice {
            Some(d) => {
                if !matches!(d.payment_method.as_str(), "prepaid" | "cod") {
                    return Err(AppError::bad("payment method must be prepaid or cod"));
                }
                Some(logistics::quote(&mut tx, supplier, &d.carrier_code, &d.delivery_type, d.payment_method == "cod", total).await?)
            }
            None if logistics::shop_has_shipping(&mut tx, supplier).await? => {
                let name: String = sqlx::query_scalar("SELECT name FROM shops WHERE id = $1").bind(supplier).fetch_one(&mut *tx).await?;
                return Err(AppError::bad(format!("choose a delivery option for {name}")));
            }
            None => None,
        };
        let cod = q.as_ref().is_some_and(|q| q.cod);
        let cod_amount = q.as_ref().filter(|q| q.cod).map(|q| total + q.charged_fee_cents + q.cod_fee_cents).unwrap_or(0);
        let order_id: Uuid = sqlx::query_scalar(
            "INSERT INTO orders (buyer_id, total_cents, currency, shipping_address, carrier_code, delivery_type, fee_payer,
                                 shipping_fee_cents, payment_method, cod_fee_cents, cod_amount_cents, cod_status)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) RETURNING id",
        )
        .bind(user.id)
        .bind(total)
        .bind(&currency)
        .bind(&req.shipping_address)
        .bind(q.as_ref().map(|q| q.carrier_code.clone()))
        .bind(q.as_ref().map(|q| q.delivery_type.as_str()).unwrap_or("branch"))
        .bind(q.as_ref().map(|q| q.fee_payer.as_str()).unwrap_or("buyer"))
        .bind(q.as_ref().map(|q| q.fee_cents).unwrap_or(0))
        .bind(if cod { "cod" } else { "prepaid" })
        .bind(q.as_ref().map(|q| q.cod_fee_cents).unwrap_or(0))
        .bind(cod_amount)
        .bind(if cod { "pending" } else { "none" })
        .fetch_one(&mut *tx)
        .await?;

        for l in lines {
            let commission = commission_of(l.p.price_cents, l.qty, l.bps);
            let item_id: Uuid = sqlx::query_scalar(
                "INSERT INTO order_items (order_id, product_id, product_name, supplier_shop_id, seller_shop_id,
                                          qty, unit_price_cents, commission_bps, commission_cents)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id",
            )
            .bind(order_id)
            .bind(l.p.id)
            .bind(&l.p.name)
            .bind(supplier)
            .bind(l.seller)
            .bind(l.qty)
            .bind(l.p.price_cents)
            .bind(l.bps)
            .bind(commission)
            .fetch_one(&mut *tx)
            .await?;

            let stock_after: i32 = sqlx::query_scalar(
                "UPDATE products SET stock = stock - $2, updated_at = now() WHERE id = $1 RETURNING stock",
            )
            .bind(l.p.id)
            .bind(l.qty)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO inventory_movements (product_id, delta, stock_after, reason, ref_order_id, created_by)
                 VALUES ($1,$2,$3,'sale',$4,$5)",
            )
            .bind(l.p.id)
            .bind(-l.qty)
            .bind(stock_after)
            .bind(order_id)
            .bind(user.id)
            .execute(&mut *tx)
            .await?;

            if let (Some(seller), true) = (l.seller, commission > 0) {
                sqlx::query(
                    "INSERT INTO commissions (order_item_id, beneficiary_shop_id, supplier_shop_id, amount_cents)
                     VALUES ($1,$2,$3,$4)",
                )
                .bind(item_id)
                .bind(seller)
                .bind(supplier)
                .bind(commission)
                .execute(&mut *tx)
                .await?;
            }
        }
        order_ids.push(order_id);
    }
    tx.commit().await?;
    Ok(Json(load_orders(&st, &order_ids).await?))
}

pub async fn my_orders(State(st): State<AppState>, user: AuthUser) -> AppResult<Json<Vec<Value>>> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM orders WHERE buyer_id=$1 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(user.id)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(load_orders(&st, &ids).await?))
}

async fn order_supplier(st: &AppState, order_id: Uuid) -> AppResult<(Uuid, String, Uuid)> {
    sqlx::query_as(
        "SELECT o.buyer_id, o.status, (SELECT supplier_shop_id FROM order_items WHERE order_id=o.id LIMIT 1)
         FROM orders o WHERE o.id=$1",
    )
    .bind(order_id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)
}

pub async fn get_order(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let (buyer, _, supplier) = order_supplier(&st, id).await?;
    if buyer != user.id && owned_shop(&st, supplier, &user).await.is_err() {
        return Err(AppError::Forbidden);
    }
    load_orders(&st, &[id])
        .await?
        .pop()
        .map(Json)
        .ok_or(AppError::NotFound)
}

/// Mock payment. Replace with a real PSP (PromptPay, Stripe, Omise…) webhook.
pub async fn pay(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let (buyer, status, _) = order_supplier(&st, id).await?;
    if buyer != user.id {
        return Err(AppError::Forbidden);
    }
    if status != "pending" {
        return Err(AppError::bad(format!("order is {status}")));
    }
    let method: String = sqlx::query_scalar("SELECT payment_method FROM orders WHERE id=$1").bind(id).fetch_one(&st.db).await?;
    if method == "cod" {
        return Err(AppError::bad("this order is paid in cash on delivery"));
    }
    sqlx::query("UPDATE orders SET status='paid', updated_at=now() WHERE id=$1")
        .bind(id)
        .execute(&st.db)
        .await?;
    load_orders(&st, &[id])
        .await?
        .pop()
        .map(Json)
        .ok_or(AppError::NotFound)
}

async fn do_cancel(tx: &mut Transaction<'_, Postgres>, order_id: Uuid, by: Uuid) -> AppResult<()> {
    restock_and_cancel(tx, order_id, by, "cancel").await
}

/// A COD parcel the customer refused came back to the shop: stock returns, order is cancelled.
pub async fn cancel_returned(tx: &mut Transaction<'_, Postgres>, order_id: Uuid, by: Uuid) -> AppResult<()> {
    restock_and_cancel(tx, order_id, by, "return").await
}

/// Delivered (or COD cash collected): complete the order and approve resale commissions.
pub async fn complete(tx: &mut Transaction<'_, Postgres>, order_id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE orders SET status='completed', updated_at=now() WHERE id=$1")
        .bind(order_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        "UPDATE commissions SET status='approved', updated_at=now()
         WHERE status='pending' AND order_item_id IN (SELECT id FROM order_items WHERE order_id=$1)",
    )
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn restock_and_cancel(tx: &mut Transaction<'_, Postgres>, order_id: Uuid, by: Uuid, reason: &str) -> AppResult<()> {
    let items: Vec<(Uuid, i32)> =
        sqlx::query_as("SELECT product_id, qty FROM order_items WHERE order_id=$1")
            .bind(order_id)
            .fetch_all(&mut **tx)
            .await?;
    for (pid, qty) in items {
        let after: i32 = sqlx::query_scalar(
            "UPDATE products SET stock = stock + $2, updated_at=now() WHERE id=$1 RETURNING stock",
        )
        .bind(pid)
        .bind(qty)
        .fetch_one(&mut **tx)
        .await?;
        sqlx::query(
            "INSERT INTO inventory_movements (product_id, delta, stock_after, reason, ref_order_id, created_by)
             VALUES ($1,$2,$3,$6,$4,$5)",
        )
        .bind(pid)
        .bind(qty)
        .bind(after)
        .bind(order_id)
        .bind(by)
        .bind(reason)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(
        "UPDATE commissions SET status='void', updated_at=now()
         WHERE order_item_id IN (SELECT id FROM order_items WHERE order_id=$1)",
    )
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    // A cancelled COD order no longer has cash to collect ('returned' is kept for refused parcels).
    sqlx::query(
        "UPDATE orders SET status='cancelled', updated_at=now(),
            cod_status = CASE WHEN cod_status = 'pending' THEN 'none' ELSE cod_status END WHERE id=$1",
    )
    .bind(order_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn cancel(
    State(st): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let (buyer, status, _) = order_supplier(&st, id).await?;
    if buyer != user.id {
        return Err(AppError::Forbidden);
    }
    if !matches!(status.as_str(), "pending" | "paid") {
        return Err(AppError::bad(format!(
            "cannot cancel an order that is {status}"
        )));
    }
    let mut tx = st.db.begin().await?;
    do_cancel(&mut tx, id, user.id).await?;
    tx.commit().await?;
    load_orders(&st, &[id])
        .await?
        .pop()
        .map(Json)
        .ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
pub struct SalesQ {
    /// "supplier" (default): orders for my products. "seller": sales I made as sell-staff.
    pub role: Option<String>,
    pub status: Option<String>,
}

pub async fn shop_sales(
    State(st): State<AppState>,
    user: AuthUser,
    Path(shop_id): Path<Uuid>,
    Query(q): Query<SalesQ>,
) -> AppResult<Json<Vec<Value>>> {
    owned_shop(&st, shop_id, &user).await?;
    let col = if q.role.as_deref() == Some("seller") {
        "seller_shop_id"
    } else {
        "supplier_shop_id"
    };
    let ids: Vec<Uuid> = sqlx::query_scalar(&format!(
        "SELECT o.id FROM orders o
         WHERE EXISTS (SELECT 1 FROM order_items oi WHERE oi.order_id = o.id AND oi.{col} = $1)
           AND ($2::text IS NULL OR o.status = $2)
         ORDER BY o.created_at DESC LIMIT 200"
    ))
    .bind(shop_id)
    .bind(q.status.filter(|s| !s.is_empty()))
    .fetch_all(&st.db)
    .await?;
    Ok(Json(load_orders(&st, &ids).await?))
}

#[derive(Deserialize)]
pub struct StatusReq {
    pub status: String,
    /// When shipping: courier (defaults to the one the buyer chose) and tracking number.
    pub carrier_code: Option<String>,
    pub tracking_no: Option<String>,
}

/// Supplier moves the order through fulfilment: paid → shipped → completed, or cancels.
pub async fn set_status(
    State(st): State<AppState>,
    user: AuthUser,
    Path((shop_id, order_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<StatusReq>,
) -> AppResult<Json<Value>> {
    owned_shop(&st, shop_id, &user).await?;
    let (_, current, supplier) = order_supplier(&st, order_id).await?;
    if supplier != shop_id {
        return Err(AppError::Forbidden);
    }
    let (method, cod_status): (String, String) =
        sqlx::query_as("SELECT payment_method, cod_status FROM orders WHERE id=$1").bind(order_id).fetch_one(&st.db).await?;
    let cod = method == "cod";
    let allowed = matches!(
        (current.as_str(), req.status.as_str()),
        ("paid", "shipped") | ("shipped", "completed") | ("pending" | "paid", "cancelled")
    ) || (cod && current == "pending" && req.status == "shipped");
    if !allowed {
        return Err(AppError::bad(format!(
            "cannot move order from {current} to {}",
            req.status
        )));
    }
    let mut tx = st.db.begin().await?;
    if req.status == "cancelled" {
        do_cancel(&mut tx, order_id, user.id).await?;
    } else if req.status == "shipped" {
        let carrier = req.carrier_code.as_deref().map(str::trim).filter(|c| !c.is_empty());
        if let Some(c) = carrier {
            let ok: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM carriers WHERE code = $1)").bind(c).fetch_one(&mut *tx).await?;
            if !ok {
                return Err(AppError::bad("unknown courier"));
            }
        }
        sqlx::query(
            "UPDATE orders SET status='shipped', shipped_at=now(), updated_at=now(),
                carrier_code = COALESCE($2, carrier_code), tracking_no = COALESCE($3, tracking_no) WHERE id=$1",
        )
        .bind(order_id)
        .bind(carrier)
        .bind(req.tracking_no.map(|t| t.trim().chars().take(60).collect::<String>()))
        .execute(&mut *tx)
        .await?;
    } else {
        // completed: delivered. For COD this means the courier collected the cash.
        complete(&mut tx, order_id).await?;
        if cod && cod_status == "pending" {
            sqlx::query("UPDATE orders SET cod_status='collected', cod_collected_at=now() WHERE id=$1")
                .bind(order_id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    load_orders(&st, &[order_id])
        .await?
        .pop()
        .map(Json)
        .ok_or(AppError::NotFound)
}
