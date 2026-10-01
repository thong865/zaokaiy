use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, patch, post, put},
};
use uuid::Uuid;

use kernel::{config::Config, Routes};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    AppState,
};

mod ads;
mod ai_routes;
mod barcodes;
pub mod catalog;
pub mod categories;
mod content;
mod inventory;
pub mod logistics;
pub mod media;
pub mod orders;
mod partners;
pub mod pos;
pub mod products;
pub mod review;
pub mod social;
pub mod webhooks;
pub mod stats;

pub use content::{
    insert as content_insert, CreateContent as CreateContentReq, KINDS as CONTENT_KINDS,
};
pub use stats::compute as stats_compute;

/// Commerce routes (mounted under /api by the gateway).
pub fn routes(cfg: &Config) -> Routes {
    let upload_limit = (cfg.media_max_video_mb.max(cfg.media_max_image_mb) + 10) * 1_048_576;
    Routes::new()
        // public catalog / storefronts
        .route("/catalog/products", get(catalog::list_products))
        .route("/catalog/products/{id}", get(catalog::get_product))
        .route("/catalog/categories", get(categories::public_tree))
        .route("/categories", get(categories::public_tree))
        .route("/storefront/{slug}", get(catalog::storefront))
        .route("/feed/contents", get(content::public_feed))
        .route("/ads/serve", get(ads::serve))
        .route("/ads/{id}/click", post(ads::click))
        .route("/shops/{id}/stats", get(stats::shop_stats))
        // products & inventory
        .route(
            "/shops/{id}/products",
            get(products::list_for_shop).post(products::create),
        )
        .route(
            "/products/{id}",
            get(products::get_owned)
                .patch(products::update)
                .delete(products::archive),
        )
        .route("/products/{id}/stock", post(inventory::adjust_stock))
        .route("/shops/{id}/inventory/movements", get(inventory::movements))
        .route("/shops/{id}/inventory/low-stock", get(inventory::low_stock))
        // reseller program
        .route("/marketplace/products", get(partners::marketplace))
        .route(
            "/shops/{id}/partnerships",
            get(partners::list).post(partners::request),
        )
        .route("/partnerships/{id}/decision", post(partners::decide))
        .route("/partnerships/{id}/revoke", post(partners::revoke))
        .route(
            "/shops/{id}/listings",
            get(partners::listings).post(partners::add_listing),
        )
        .route("/listings/{id}", delete(partners::remove_listing))
        .route("/shops/{id}/commissions", get(partners::commissions_earned))
        .route(
            "/shops/{id}/commissions/payable",
            get(partners::commissions_payable),
        )
        .route("/commissions/{id}/pay", post(partners::pay_commission))
        .route("/media/config", get(media::config))
        .route("/admin/media/backfill", post(media::admin_backfill))
        // delivery couriers + COD
        .route("/carriers", get(logistics::carriers))
        .route("/shipping/options", get(logistics::public_options))
        .route("/shops/{id}/shipping", get(logistics::shop_shipping))
        .route("/shops/{id}/shipping/{carrier}", put(logistics::set_shop_shipping))
        .route("/shops/{id}/cod", get(logistics::cod_ledger))
        .route("/shops/{id}/cod/update", post(logistics::cod_update))
        .route("/admin/carriers", get(logistics::admin_carriers).post(logistics::admin_create_carrier))
        .route("/admin/carriers/{code}", patch(logistics::admin_update_carrier))
        // orders
        .route("/orders/checkout", post(orders::checkout))
        .route("/orders", get(orders::my_orders))
        .route("/orders/{id}", get(orders::get_order))
        .route("/orders/{id}/pay", post(orders::pay))
        .route("/orders/{id}/cancel", post(orders::cancel))
        .route("/shops/{id}/sales", get(orders::shop_sales))
        .route(
            "/shops/{id}/orders/{order_id}/status",
            post(orders::set_status),
        )
        // ads
        .route("/shops/{id}/ads", get(ads::list).post(ads::create))
        .route("/ads/{id}", patch(ads::update))
        // creator content
        .route(
            "/shops/{id}/contents",
            get(content::list).post(content::create),
        )
        .route(
            "/contents/{id}",
            patch(content::update).delete(content::remove),
        )
        // AI
        .route("/ai/status", get(ai_routes::status))
        .route("/shops/{id}/ai/generate", post(ai_routes::generate))
        .route("/shops/{id}/ai/agent", post(ai_routes::agent))
        // categories (marketplace: admin · shop: owner)
        .route("/shops/{id}/categories", get(categories::shop_tree).post(categories::create_shop_category))
        .route("/categories/reorder", post(categories::reorder))
        .route("/categories/{id}", patch(categories::update).delete(categories::remove))
        // product review
        .route("/products/{id}/submit", post(review::submit))
        .route("/products/{id}/reviews", get(review::history))
        // admin console
        .route("/admin/overview", get(review::overview))
        .route("/admin/products", get(review::queue))
        .route("/admin/products/review", post(review::decide_bulk))
        .route("/admin/products/{id}", get(review::detail))
        .route("/admin/products/{id}/review", post(review::decide))
        .route("/admin/shops", get(review::shops))
        .route("/admin/shops/{id}", patch(review::patch_shop))
        .route("/admin/categories", get(categories::admin_tree).post(categories::create_marketplace_category))
        // point of sale
        .route("/shops/{id}/pos/products", get(pos::search))
        .route("/shops/{id}/pos/scan", get(barcodes::scan))
        .route("/shops/{id}/barcodes/generate", post(barcodes::generate))
        .route("/shops/{id}/pos/sales", get(pos::list_sales).post(pos::create_sale))
        .route("/shops/{id}/pos/summary", get(pos::summary))
        .route("/pos/sales/{id}", get(pos::get_sale))
        .route("/pos/sales/{id}/void", post(pos::void_sale))
        .route("/pos/sales/{id}/invoice", post(pos::issue_invoice))
        .route("/orders/{id}/document", get(pos::order_document))
        // social commerce
        .route("/shops/{id}/social/settings", get(social::settings).patch(social::update_settings))
        .route("/shops/{id}/social/channels", get(social::list_channels).post(social::create_channel))
        .route("/social/channels/{id}", patch(social::update_channel).delete(social::delete_channel))
        .route("/shops/{id}/social/sessions", get(social::sessions).post(social::start_session))
        .route("/social/sessions/{id}/end", post(social::end_session))
        .route("/shops/{id}/social/feed", get(social::feed))
        .route("/shops/{id}/social/board", get(social::board))
        .route("/shops/{id}/social/simulate", post(social::simulate))
        .route("/shops/{id}/social/orders", get(social::list_orders))
        .route("/shops/{id}/social/assign-codes", post(social::assign_codes))
        .route("/social/orders/{id}", get(social::get_order))
        .route("/social/orders/{id}/status", post(social::set_status))
        .route("/social/orders/{id}/items", post(social::set_item))
        .route("/public/social-orders/{token}", get(social::public_get))
        .route("/public/social-orders/{token}/confirm", post(social::public_confirm))
        .route("/public/social-orders/{token}/payment", post(social::public_payment))
        .route("/webhooks/meta", get(webhooks::meta_verify).post(webhooks::meta_receive))
        .route("/webhooks/tiktok", post(webhooks::tiktok_receive))
        .route("/webhooks/ingest/{channel_id}", post(webhooks::ingest))
        // media library
        .route("/shops/{id}/media", get(media::list))
        .route(
            "/shops/{id}/media/upload",
            post(media::upload).layer(DefaultBodyLimit::max(upload_limit)),
        )
        .route("/shops/{id}/media/url", post(media::add_url))
        .route("/shops/{id}/media/delete", post(media::bulk_remove))
        .route("/media/{id}", get(media::get_one).patch(media::update).delete(media::remove))
        .route("/products/{id}/media", get(media::get_gallery).put(media::set_gallery))
        // POS open bills (several sales at once, shared between tills)
        .route("/shops/{id}/pos/bills", get(pos::list_bills).post(pos::create_bill))
        .route("/pos/bills/{id}", get(pos::get_bill).put(pos::save_bill).delete(pos::delete_bill))
}

pub use kernel::guards::{owned_shop, owner_shop};

/// Load a product and ensure the caller owns its shop.
pub async fn owned_product(
    state: &AppState,
    product_id: Uuid,
    user: &AuthUser,
) -> AppResult<crate::models::Product> {
    let p: crate::models::Product = sqlx::query_as("SELECT * FROM products WHERE id = $1")
        .bind(product_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    owned_shop(state, p.shop_id, user).await?;
    Ok(p)
}

pub fn commission_of(unit_price: i64, qty: i32, bps: i32) -> i64 {
    unit_price * qty as i64 * bps as i64 / 10_000
}
