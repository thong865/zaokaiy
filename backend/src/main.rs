mod ai;
mod auth;
mod config;
mod error;
mod i18n;
mod media;
mod models;
mod routes;
mod social_parser;
mod storage;

use std::{sync::Arc, time::Duration};

use axum::http::{header, HeaderValue, Method};
use sqlx::postgres::PgPoolOptions;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub cfg: Arc<config::Config>,
    pub http: reqwest::Client,
    pub storage: storage::Storage,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,zaokaiy_api=debug".into()),
        )
        .init();

    let cfg = config::Config::from_env();
    let db = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&cfg.database_url)
        .await
        .expect("connect to database");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("run migrations");

    if !cfg.admin_emails.is_empty() {
        let promoted = sqlx::query("UPDATE users SET role = 'admin' WHERE lower(email) = ANY($1) AND role <> 'admin'")
            .bind(&cfg.admin_emails)
            .execute(&db)
            .await
            .expect("promote admins")
            .rows_affected();
        if promoted > 0 {
            tracing::info!(promoted, "promoted ADMIN_EMAILS users to admin");
        }
    }

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("http client");

    let origins: Vec<HeaderValue> = cfg
        .cors_origins
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .max_age(Duration::from_secs(3600));

    let bind = cfg.bind_addr.clone();
    if cfg.anthropic_api_key.is_none() {
        tracing::warn!("ANTHROPIC_API_KEY not set — AI features use offline templates");
    }
    let storage = storage::Storage::from_config(&cfg);
    let state = AppState {
        db,
        cfg: Arc::new(cfg),
        http,
        storage,
    };

    // Background: expire unpaid social orders and release their stock.
    {
        let st = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(60));
            loop {
                tick.tick().await;
                match routes::social::expire_due(&st).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(n, "expired social orders"),
                    Err(e) => tracing::warn!(error = %e, "social expiry job failed"),
                }
            }
        });
    }

    let app = routes::router(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    tracing::info!("zaokaiy api listening on http://{bind}");
    axum::serve(listener, app).await.expect("server");
}
