//! Insurance core: agent / broker shops publish plans (motor, health, travel …); customers get an
//! instant quote and apply online; the agent reviews, collects the premium and issues the policy.

pub use kernel::{auth, error, AppState};

mod lao;
mod routes;

use axum::routing::{get, post};
use kernel::{config::Config, CoreModule, Routes};
use sqlx::migrate::Migrator;

pub use routes::premium;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub fn module() -> CoreModule {
    CoreModule {
        verticals: &["insurance"],
        migrator: Some(&MIGRATOR),
        lao: lao::EXACT,
        lao_patterns: lao::PATTERNS,
        access_rules: ACCESS,
        ..CoreModule::new("insurance", "Insurance", "ປະກັນໄພ", "shield-plus", routes)
    }
}

/// Staff permissions: applications = orders, plans = products.
const ACCESS: &[(&str, &str, &str)] = &[
    ("*", "/shops/{id}/insurance/applications", "orders"),
    ("*", "/insurance/applications/*", "orders"),
    ("*", "/shops/{id}/insurance/plans", "products"),
    ("PATCH", "/insurance/plans/{id}", "products"),
];

fn routes(_cfg: &Config) -> Routes {
    Routes::new()
        // public
        .route("/insurance/plans", get(routes::public_plans))
        .route("/insurance/plans/{id}", get(routes::public_plan).patch(routes::update_plan))
        .route("/insurance/plans/{id}/quote", post(routes::quote))
        .route("/insurance/plans/{id}/apply", post(routes::apply))
        .route("/me/insurance", get(routes::my_applications))
        // agent
        .route("/shops/{id}/insurance/plans", get(routes::shop_plans).post(routes::create_plan))
        .route("/shops/{id}/insurance/applications", get(routes::shop_applications))
        .route("/insurance/applications/{id}/decision", post(routes::decide))
}
