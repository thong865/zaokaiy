//! Finance core: VAT and e-commerce tax, with the platform owner as the shops' tax agent.
//!
//! * A shop owner reads the tax-agent policy and accepts it (`POST /shops/{id}/finance/mandate`).
//! * Every month the admin generates filings: each mandated shop's completed sales, collected from
//!   every core through the registry's [`TaxSource`](kernel::core::TaxSource)s (commerce orders,
//!   social orders, POS, restaurant orders …), turned into VAT + e-commerce tax.
//! * The admin submits each filing to the tax office (reference number), then marks it paid.
//!   Shops see their statements and a live preview of the current month.

pub use kernel::{auth, error, AppState};

mod calc;
mod lao;
mod routes;

use axum::routing::{get, post, put};
use kernel::{config::Config, CoreModule, Routes};
use sqlx::migrate::Migrator;

pub use calc::{compute, Taxes};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub fn module() -> CoreModule {
    CoreModule {
        migrator: Some(&MIGRATOR),
        lao: lao::EXACT,
        lao_patterns: lao::PATTERNS,
        ..CoreModule::new("finance", "Tax & VAT", "ອາກອນ ແລະ VAT", "landmark", routes)
    }
}

fn routes(_cfg: &Config) -> Routes {
    Routes::new()
        .route("/finance/policy", get(routes::policy))
        .route("/shops/{id}/finance", get(routes::shop_overview))
        .route("/shops/{id}/finance/mandate", post(routes::accept_mandate))
        .route("/shops/{id}/finance/mandate/revoke", post(routes::revoke_mandate))
        .route("/admin/finance/settings", get(routes::admin_settings).put(routes::admin_save_settings))
        .route("/admin/finance/filings", get(routes::admin_filings))
        .route("/admin/finance/filings/generate", post(routes::admin_generate))
        .route("/admin/finance/filings/export", get(routes::admin_export))
        .route("/admin/finance/filings/{id}", put(routes::admin_update_filing))
}
