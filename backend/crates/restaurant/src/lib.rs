//! Restaurant core: menu (sections + dishes), tables with QR codes, and a kitchen board for
//! dine-in, takeaway and delivery orders placed from the public menu.

pub use kernel::{auth, error, AppState};

mod lao;
mod routes;

use axum::routing::{get, patch, post};
use chrono::NaiveDate;
use kernel::{
    config::Config,
    core::{TaxSource, TaxableLine},
    error::AppResult,
    BoxFut, CoreModule, Registry, Routes,
};
use sqlx::migrate::Migrator;
use uuid::Uuid;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub fn module() -> CoreModule {
    CoreModule {
        verticals: &["restaurant"],
        migrator: Some(&MIGRATOR),
        install,
        lao: lao::EXACT,
        lao_patterns: lao::PATTERNS,
        access_rules: ACCESS,
        ..CoreModule::new("restaurant", "Food", "ອາຫານ", "utensils-crossed", routes)
    }
}

/// Staff permissions: kitchen board = orders, menu = products, tables + ordering settings = settings.
const ACCESS: &[(&str, &str, &str)] = &[
    ("*", "/shops/{id}/restaurant/orders", "orders"),
    ("*", "/restaurant/orders/{id}", "orders"),
    ("*", "/shops/{id}/restaurant/sections", "products"),
    ("*", "/shops/{id}/restaurant/items", "products"),
    ("*", "/restaurant/sections/{id}", "products"),
    ("*", "/restaurant/items/{id}", "products"),
    ("*", "/shops/{id}/restaurant/tables", "settings"),
    ("*", "/restaurant/tables/{id}", "settings"),
    ("PUT", "/shops/{id}/restaurant", "settings"),
    ("GET", "/shops/{id}/restaurant", "stats"),
];

fn routes(_cfg: &Config) -> Routes {
    Routes::new()
        // public
        .route("/restaurant/places", get(routes::places))
        .route("/restaurant/menu/{slug}", get(routes::public_menu))
        .route("/restaurant/menu/{slug}/orders", post(routes::place_order))
        .route("/restaurant/track/{token}", get(routes::track))
        // seller
        .route("/shops/{id}/restaurant", get(routes::overview).put(routes::save_settings))
        .route("/shops/{id}/restaurant/sections", post(routes::create_section))
        .route("/restaurant/sections/{id}", patch(routes::update_section).delete(routes::delete_section))
        .route("/shops/{id}/restaurant/items", post(routes::create_item))
        .route("/restaurant/items/{id}", patch(routes::update_item).delete(routes::delete_item))
        .route("/shops/{id}/restaurant/tables", post(routes::create_table))
        // GET takes the table's QR token (public); PATCH / DELETE take its id (seller)
        .route("/restaurant/tables/{id}", get(routes::public_table).patch(routes::update_table).delete(routes::delete_table))
        .route("/shops/{id}/restaurant/orders", get(routes::board))
        .route("/restaurant/orders/{id}", patch(routes::update_order))
}

fn install(reg: &mut Registry) {
    reg.add_tax_source(PaidOrders);
}

/// Paid restaurant orders that were not cancelled (service charge included).
struct PaidOrders;

impl TaxSource for PaidOrders {
    fn key(&self) -> &'static str {
        "restaurant.orders"
    }
    fn label(&self) -> &'static str {
        "Restaurant orders"
    }
    fn taxable<'a>(&'a self, st: &'a AppState, from: NaiveDate, to: NaiveDate, shop: Option<Uuid>) -> BoxFut<'a, AppResult<Vec<TaxableLine>>> {
        Box::pin(async move {
            let rows: Vec<(Uuid, String, i64, i64)> = sqlx::query_as(
                "SELECT shop_id, currency, SUM(total_cents)::bigint, COUNT(*) FROM restaurant.orders
                 WHERE paid AND status <> 'cancelled' AND day >= $1 AND day < $2 AND ($3::uuid IS NULL OR shop_id = $3)
                 GROUP BY 1, 2",
            )
            .bind(from)
            .bind(to)
            .bind(shop)
            .fetch_all(&st.db)
            .await?;
            Ok(rows.into_iter().map(|(shop_id, currency, gross_cents, count)| TaxableLine { shop_id, currency, gross_cents, count }).collect())
        })
    }
}
