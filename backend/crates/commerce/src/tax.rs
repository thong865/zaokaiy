//! Commerce sales for the finance core's tax filings (amounts as charged, VAT included if any).

use chrono::NaiveDate;
use kernel::{
    core::{TaxSource, TaxableLine},
    error::AppResult,
    AppState, BoxFut,
};
use uuid::Uuid;


async fn lines(st: &AppState, sql: &str, from: NaiveDate, to: NaiveDate, shop: Option<Uuid>) -> AppResult<Vec<TaxableLine>> {
    let rows: Vec<(Uuid, String, i64, i64)> = sqlx::query_as(sql).bind(from).bind(to).bind(shop).fetch_all(&st.db).await?;
    Ok(rows
        .into_iter()
        .map(|(shop_id, currency, gross_cents, count)| TaxableLine { shop_id, currency, gross_cents, count })
        .collect())
}

/// Online orders (paid, shipped or completed), goods value per supplier shop. Shipping is excluded.
pub struct Orders;
impl TaxSource for Orders {
    fn key(&self) -> &'static str {
        "commerce.orders"
    }
    fn label(&self) -> &'static str {
        "Online orders"
    }
    fn taxable<'a>(&'a self, st: &'a AppState, from: NaiveDate, to: NaiveDate, shop: Option<Uuid>) -> BoxFut<'a, AppResult<Vec<TaxableLine>>> {
        Box::pin(lines(
            st,
            // Goods, less discounts the shop itself funded (coupons/points of the promo module;
            // platform-funded discounts are still the shop's sale).
            "SELECT x.shop, x.currency, SUM(x.goods - x.disc)::bigint, COUNT(*)
             FROM (SELECT (SELECT oi.supplier_shop_id FROM order_items oi WHERE oi.order_id = o.id LIMIT 1) AS shop, o.currency,
                          (SELECT COALESCE(SUM(oi.unit_price_cents * oi.qty), 0) FROM order_items oi WHERE oi.order_id = o.id) AS goods,
                          (SELECT COALESCE(SUM(r.discount_cents - r.platform_cents
                                  - CASE WHEN k.reward_type = 'free_shipping' AND k.shop_id IS NOT NULL THEN r.coupon_cents ELSE 0 END), 0)
                           FROM promo_redemptions r LEFT JOIN promo_coupons k ON k.id = r.coupon_id
                           WHERE r.ref_type = 'order' AND r.ref_id = o.id AND r.reversed_at IS NULL) AS disc
                   FROM orders o
                   WHERE o.status IN ('paid','shipped','completed')
                     AND (o.created_at AT TIME ZONE 'Asia/Vientiane')::date >= $1 AND (o.created_at AT TIME ZONE 'Asia/Vientiane')::date < $2) x
             WHERE ($3::uuid IS NULL OR x.shop = $3)
             GROUP BY 1, 2",
            from,
            to,
            shop,
        ))
    }
}

/// Comment / chat orders (paid, shipped or completed), goods subtotal.
pub struct SocialOrders;
impl TaxSource for SocialOrders {
    fn key(&self) -> &'static str {
        "commerce.social"
    }
    fn label(&self) -> &'static str {
        "Social orders"
    }
    fn taxable<'a>(&'a self, st: &'a AppState, from: NaiveDate, to: NaiveDate, shop: Option<Uuid>) -> BoxFut<'a, AppResult<Vec<TaxableLine>>> {
        Box::pin(lines(
            st,
            "SELECT shop_id, currency,
                    SUM(subtotal_cents - (SELECT COALESCE(SUM(r.discount_cents - r.platform_cents
                            - CASE WHEN k.reward_type = 'free_shipping' AND k.shop_id IS NOT NULL THEN r.coupon_cents ELSE 0 END), 0)
                        FROM promo_redemptions r LEFT JOIN promo_coupons k ON k.id = r.coupon_id
                        WHERE r.ref_type = 'social' AND r.ref_id = social_orders.id AND r.reversed_at IS NULL))::bigint, COUNT(*)
             FROM social_orders
             WHERE status IN ('paid','shipped','completed')
               AND (created_at AT TIME ZONE 'Asia/Vientiane')::date >= $1 AND (created_at AT TIME ZONE 'Asia/Vientiane')::date < $2
               AND ($3::uuid IS NULL OR shop_id = $3)
             GROUP BY 1, 2",
            from,
            to,
            shop,
        ))
    }
}

/// Counter (POS) sales that were not voided.
pub struct PosSales;
impl TaxSource for PosSales {
    fn key(&self) -> &'static str {
        "commerce.pos"
    }
    fn label(&self) -> &'static str {
        "Counter sales (POS)"
    }
    fn taxable<'a>(&'a self, st: &'a AppState, from: NaiveDate, to: NaiveDate, shop: Option<Uuid>) -> BoxFut<'a, AppResult<Vec<TaxableLine>>> {
        Box::pin(lines(
            st,
            // Platform-funded discounts (promo module) are paid to the shop, so they are still its sale.
            "SELECT shop_id, currency, SUM(total_cents + platform_discount_cents)::bigint, COUNT(*)
             FROM pos_sales
             WHERE status = 'completed'
               AND (created_at AT TIME ZONE 'Asia/Vientiane')::date >= $1 AND (created_at AT TIME ZONE 'Asia/Vientiane')::date < $2
               AND ($3::uuid IS NULL OR shop_id = $3)
             GROUP BY 1, 2",
            from,
            to,
            shop,
        ))
    }
}
