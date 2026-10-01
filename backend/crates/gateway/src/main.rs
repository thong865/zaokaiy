//! zaokaiy gateway — the `zaokaiy-api` binary.
//!
//! Runs any set of cores in-process and proxies the others:
//!
//! * `ZK_CORES=all` (default) — every core in one process (single VPS).
//! * `ZK_CORES=platform,commerce,finance` + `ZK_REMOTE_VEHICLE=http://vehicle:8080` — vehicle routes
//!   are forwarded to another instance started with `ZK_CORES=vehicle`.
//!
//! Every instance shares the database and `JWT_SECRET`, so a core authenticates requests itself.

mod proxy;

use std::{borrow::Cow, sync::Arc, time::Duration};

use axum::{
    http::{header, HeaderValue, Method},
    routing::{any, get},
    Json, Router,
};
use kernel::{
    config::Config,
    core::CoreInfo,
    crypto::Sealer,
    storage::Storage,
    AppState, CoreModule, Registry,
};
use serde_json::json;
use sqlx::{migrate::Migrator, postgres::PgPoolOptions};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::ServeDir, set_header::SetResponseHeaderLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

/// Every core compiled into this binary.
fn all_modules() -> Vec<CoreModule> {
    vec![
        platform::module(),
        commerce::module(),
        vehicle::module(),
        restaurant::module(),
        insurance::module(),
        finance::module(),
    ]
}

/// Where each core runs, from `ZK_CORES` and `ZK_REMOTE_<NAME>`.
struct Placement {
    module: CoreModule,
    /// None = in this process.
    remote: Option<String>,
}

fn placements() -> Vec<Placement> {
    let wanted = std::env::var("ZK_CORES").unwrap_or_else(|_| "all".into());
    let wanted: Vec<String> = wanted.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()).collect();
    let all = wanted.iter().any(|w| w == "all");
    all_modules()
        .into_iter()
        .filter_map(|module| {
            let remote = std::env::var(format!("ZK_REMOTE_{}", module.name.to_uppercase()))
                .ok()
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty());
            let local = all || wanted.iter().any(|w| w == module.name);
            (local || remote.is_some()).then_some(Placement { module, remote })
        })
        .collect()
}

/// Kernel baseline + the migrations of the cores running here, as one ordered set.
fn migrator(local: &[&CoreModule]) -> Migrator {
    let mut migrations: Vec<_> = kernel::MIGRATOR.migrations.iter().cloned().collect();
    for m in local {
        if let Some(mig) = m.migrator {
            migrations.extend(mig.migrations.iter().cloned());
        }
    }
    migrations.sort_by_key(|m| m.version);
    Migrator {
        migrations: Cow::Owned(migrations),
        // Other instances may have applied migrations of cores that don't run here.
        ignore_missing: true,
        locking: true,
        no_tx: false,
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,zaokaiy_api=debug".into()))
        .init();

    let cfg = Config::from_env();
    let placed = placements();
    assert!(!placed.is_empty(), "ZK_CORES selects no core");
    let local: Vec<&CoreModule> = placed.iter().filter(|p| p.remote.is_none()).map(|p| &p.module).collect();

    let db = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&cfg.database_url)
        .await
        .expect("connect to database");
    migrator(&local).run(&db).await.expect("run migrations");

    if local.iter().any(|m| m.name == "platform") && !cfg.admin_emails.is_empty() {
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

    // Registry: every enabled core registers its extension points (they share the database).
    let mut reg = Registry::default();
    for m in all_modules() {
        kernel::i18n::register_lao(m.lao, m.lao_patterns);
        if let Some(f) = m.lao_fn {
            kernel::i18n::register_lao_fn(f);
        }
        kernel::access::register_rules(m.access_rules);
    }
    for p in &placed {
        reg.cores.push(CoreInfo {
            name: p.module.name,
            title: p.module.title,
            title_lo: p.module.title_lo,
            icon: p.module.icon,
            verticals: p.module.verticals,
            served_by: p.remote.clone().unwrap_or_else(|| "local".into()),
        });
        (p.module.install)(&mut reg);
    }
    reg.finish();

    let http = reqwest::Client::builder().timeout(Duration::from_secs(120)).build().expect("http client");
    let origins: Vec<HeaderValue> = cfg.cors_origins.iter().filter_map(|o| o.parse().ok()).collect();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        .max_age(Duration::from_secs(3600));

    let bind = cfg.bind_addr.clone();
    if cfg.ai_api_key.is_none() {
        tracing::warn!("OPENROUTER_API_KEY not set — AI features use offline templates");
    } else {
        tracing::info!(model = %cfg.ai_model, fallbacks = ?cfg.ai_fallback_models, "AI via OpenRouter");
    }
    let storage = Storage::from_config(&cfg);
    let private = Storage::private_from_config(&cfg);
    let sealer = Sealer::from_config(&cfg.kyc_encryption_key, &cfg.jwt_secret);
    let state = AppState { db, cfg: Arc::new(cfg), http: http.clone(), storage, private, sealer, reg: Arc::new(reg) };

    // Routes: local cores mounted, remote cores proxied on the same paths.
    let cores = state.reg.cores.clone();
    let mut api: Router<AppState> = Router::new()
        .route("/health", get(|| async { Json(json!({ "ok": true })) }))
        .route("/cores", get(move || async move { Json(json!({ "cores": cores })) }));
    for p in &placed {
        let routes = (p.module.routes)(&state.cfg);
        match &p.remote {
            None => {
                tracing::info!(core = p.module.name, routes = routes.paths().len(), "core running here");
                api = api.merge(routes.into_router());
            }
            Some(base) => {
                tracing::info!(core = p.module.name, %base, "core proxied");
                for path in routes.paths() {
                    let target = proxy::Target::new(base.clone(), http.clone());
                    api = api.route(path, any(move |req| proxy::forward(target.clone(), req)));
                }
            }
        }
    }
    // Staff permissions: which permission the matched route needs (kernel::access).
    let api = api.route_layer(axum::middleware::from_fn(kernel::access::scope_permission));
    for m in &local {
        (m.start)(state.clone());
    }

    let mut app = Router::new().nest("/api", api);
    // Local storage driver: serve uploaded files. Keys are random UUIDs, so cache forever.
    if let Some(dir) = state.storage.local_dir() {
        let files = ServiceBuilder::new()
            .layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable")))
            .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
            .service(ServeDir::new(dir));
        app = app.nest_service("/media", files);
    }
    // Lao error messages for `Accept-Language: lo` (error responses only; successes untouched).
    let app = app
        .layer(axum::middleware::from_fn(kernel::i18n::localize_errors))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&bind).await.expect("bind");
    tracing::info!("zaokaiy api listening on http://{bind}");
    axum::serve(listener, app).await.expect("server");
}
