//! # COD risk — report customers who don't take their cash-on-delivery parcel
//!
//! 1. A COD parcel comes back refused (`cod_status = returned` in the COD ledger).
//! 2. The seller files a **report** for that order (reason + note) within
//!    `COD_RISK_REPORT_WINDOW_DAYS`. One report per order.
//! 3. An **admin** reviews it (confirm / dismiss) and decides the customer's **risk level**:
//!    `none · low · medium · high · blocked`, optionally with an expiry. The API suggests a level
//!    from the customer's confirmed reports, but never sets one by itself.
//! 4. Sellers see the level of the customers of their unshipped COD orders ("before you ship") and
//!    can look up a phone number. Checkout refuses COD for customers at or above
//!    `COD_RISK_BLOCK_COD_AT` (default `blocked`); paying online always works.
//! 5. Every filing, decision and level change is appended to a **hash-chained ledger**
//!    (`cod_risk_blocks`). Batches of blocks are **anchored** by publishing their Merkle root to a
//!    blockchain through a gateway (`COD_RISK_ANCHOR=webhook`), so nobody — including the
//!    platform — can quietly rewrite a customer's history. The ledger holds keyed hashes only.
//!
//! Customers are identified by `HMAC-SHA256(COD_RISK_PEPPER, phone in E.164)`, so the same
//! person is recognised across shops and couriers without the ledger containing their number.

use std::{sync::LazyLock, time::Duration};

use axum::{
    routing::{get, post},
};
use serde_json::{json, Value};
use sqlx::PgConnection;

use crate::{error::AppResult, AppState};

mod admin;
pub mod chain;
pub mod i18n;
pub mod identity;
pub mod risk;
mod seller;

pub use risk::Level;

/// Module settings, read once from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: bool,
    /// HMAC key for customer keys. Set COD_RISK_PEPPER in production and never change it
    /// (changing it starts every customer's history from zero).
    pub pepper: Vec<u8>,
    /// Checkout refuses COD at or above this level (None = never).
    pub block_cod_at: Option<Level>,
    /// How long after the parcel came back a report can still be filed.
    pub report_window_days: i64,
    /// `none` (local hash chain only) or `webhook` (POST the Merkle root to a chain gateway).
    pub anchor: String,
    pub anchor_url: String,
    pub anchor_secret: String,
    pub anchor_every_secs: u64,
    /// Name of the ledger sent to the gateway (lets one gateway serve several ledgers).
    pub ledger_name: String,
}

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string()).trim().to_string();
        let pepper = match get("COD_RISK_PEPPER", "") {
            p if !p.is_empty() => p.into_bytes(),
            _ => {
                use sha2::{Digest, Sha256};
                let jwt = get("JWT_SECRET", "dev-secret-change-me");
                Sha256::digest(format!("zaokaiy-cod-risk-pepper|{jwt}")).to_vec()
            }
        };
        let block = get("COD_RISK_BLOCK_COD_AT", "blocked").to_lowercase();
        Self {
            enabled: get("COD_RISK_ENABLED", "true").to_lowercase() != "false",
            pepper,
            block_cod_at: if block == "off" || block == "never" { None } else { Level::parse(&block).filter(|l| *l != Level::None).or(Some(Level::Blocked)) },
            report_window_days: get("COD_RISK_REPORT_WINDOW_DAYS", "30").parse().unwrap_or(30).clamp(1, 365),
            anchor: get("COD_RISK_ANCHOR", "none").to_lowercase(),
            anchor_url: get("COD_RISK_ANCHOR_URL", ""),
            anchor_secret: get("COD_RISK_ANCHOR_SECRET", ""),
            anchor_every_secs: get("COD_RISK_ANCHOR_EVERY_SECS", "300").parse().unwrap_or(300).max(10),
            ledger_name: get("COD_RISK_LEDGER_NAME", "zaokaiy-cod-risk"),
        }
    }

    pub fn anchoring(&self) -> bool {
        self.anchor == "webhook" && !self.anchor_url.is_empty()
    }
}

static CFG: LazyLock<Config> = LazyLock::new(Config::from_env);

pub fn cfg() -> &'static Config {
    &CFG
}

pub fn enabled() -> bool {
    cfg().enabled
}

pub fn public_status() -> Value {
    let c = cfg();
    json!({
        "enabled": c.enabled,
        "block_cod_at": c.block_cod_at.map(|l| l.as_str()),
        "report_window_days": c.report_window_days,
        "anchoring": c.enabled && c.anchoring(),
    })
}

pub fn router() -> kernel::Routes {
    kernel::Routes::new()
        // seller
        .route("/shops/{id}/cod-risk/refused", get(seller::refused))
        .route("/shops/{id}/cod-risk/unshipped", get(seller::unshipped))
        .route("/shops/{id}/cod-risk/reports", get(seller::my_reports).post(seller::file_report))
        .route("/shops/{id}/cod-risk/check", post(seller::check))
        .route("/cod-risk/reports/{id}/withdraw", post(seller::withdraw))
        // admin
        .route("/admin/cod-risk/overview", get(admin::overview))
        .route("/admin/cod-risk/reports", get(admin::reports))
        .route("/admin/cod-risk/reports/{id}/decision", post(admin::decide))
        .route("/admin/cod-risk/customers", get(admin::customers))
        .route("/admin/cod-risk/customers/{key}", get(admin::customer))
        .route("/admin/cod-risk/customers/{key}/level", post(admin::set_level))
        .route("/admin/cod-risk/chain", get(admin::chain_status))
        .route("/admin/cod-risk/chain/anchor", post(admin::anchor_now))
        .route("/admin/cod-risk/proof/{height}", get(admin::proof))
}

pub fn spawn_jobs(st: AppState) {
    let c = cfg();
    tracing::info!(anchor = %c.anchor, block_cod_at = ?c.block_cod_at, "cod_risk module enabled");
    if !c.anchoring() {
        if c.anchor != "none" {
            tracing::warn!("COD_RISK_ANCHOR={} needs COD_RISK_ANCHOR_URL — ledger is local only", c.anchor);
        }
        return;
    }
    let every = Duration::from_secs(c.anchor_every_secs);
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(every);
        tick.tick().await; // first tick is immediate; wait one period
        loop {
            tick.tick().await;
            match chain::anchor_pending(&st).await {
                Ok(Some(a)) => tracing::info!(from = a.from_height, to = a.to_height, tx = %a.tx_ref, status = %a.status, "cod_risk anchor"),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "cod_risk anchor job failed"),
            }
        }
    });
}

/// Checkout hook: refuse cash on delivery for a customer whose effective level is at or above
/// `COD_RISK_BLOCK_COD_AT`. No-op when the module is off or the phone can't be parsed.
pub async fn guard_cod(conn: &mut PgConnection, phone: Option<&str>) -> AppResult<()> {
    let c = cfg();
    let (true, Some(threshold), Some(phone)) = (c.enabled, c.block_cod_at, phone.and_then(identity::normalize)) else {
        return Ok(());
    };
    let key = identity::customer_key(&phone);
    let level = risk::effective_level(&mut *conn, &key).await?;
    if level >= threshold {
        return Err(crate::error::AppError::bad("cash on delivery is not available for this phone number — please pay online"));
    }
    Ok(())
}
