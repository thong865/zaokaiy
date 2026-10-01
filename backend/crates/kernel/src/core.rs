//! The core registry: how a core plugs into the super app.
//!
//! * [`Routes`] — a core's HTTP routes. The gateway mounts them, or (remote mode) registers the same
//!   paths as a proxy to the instance that runs the core.
//! * [`ListingExt`] — a core that adds structured data to commerce listings (vehicle spec sheets).
//!   Commerce never imports the core; it asks the registry.
//! * [`ShopHook`] — reactions to platform events about a shop (verification revoked → pause sales).
//! * [`TaxSource`] — a core's taxable sales, read by the finance core for VAT / tax filings.
//!
//! All cores share one PostgreSQL database; newer cores keep their tables in their own schema
//! (`restaurant.*`, `insurance.*`, `finance.*`) so they can be moved to their own database later.

use std::{future::Future, pin::Pin, sync::Arc};

use axum::{routing::MethodRouter, Router};
use chrono::NaiveDate;
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::{migrate::Migrator, PgConnection};
use uuid::Uuid;

use crate::{config::Config, error::AppResult, AppState};

pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A core's routes, with their paths remembered so the gateway can proxy them.
#[derive(Default)]
pub struct Routes {
    items: Vec<(&'static str, MethodRouter<AppState>)>,
}

impl Routes {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn route(mut self, path: &'static str, method: MethodRouter<AppState>) -> Self {
        self.items.push((path, method));
        self
    }
    /// Append another set of routes (e.g. an optional plug-in module).
    pub fn merge(mut self, other: Routes) -> Self {
        self.items.extend(other.items);
        self
    }
    pub fn paths(&self) -> Vec<&'static str> {
        self.items.iter().map(|(p, _)| *p).collect()
    }
    pub fn into_router(self) -> Router<AppState> {
        self.items.into_iter().fold(Router::new(), |r, (p, m)| r.route(p, m))
    }
}

/// Structured data a core attaches to commerce listings (products), under `key` in the product JSON.
pub trait ListingExt: Send + Sync {
    /// JSON field on product create/update bodies and on catalog cards (e.g. "vehicle").
    fn key(&self) -> &'static str;
    /// Correlated SQL subquery on product alias `p` returning the card summary as jsonb (or NULL).
    fn card_sql(&self) -> &'static str;
    /// Validate + normalise the input before the product transaction starts.
    fn clean(&self, input: Value) -> AppResult<Value>;
    /// Store the (cleaned) input for a product, inside the product transaction.
    fn save<'a>(&'a self, conn: &'a mut PgConnection, product_id: Uuid, input: &'a Value) -> BoxFut<'a, AppResult<()>>;
    /// Full public detail for the product page, if the product carries this extension.
    fn detail<'a>(&'a self, st: &'a AppState, product_id: Uuid) -> BoxFut<'a, AppResult<Option<Value>>>;
    /// Name of the first product (among `product_ids`) that can't be bought online right now, if any.
    fn blocks_checkout<'a>(&'a self, _conn: &'a mut PgConnection, _product_ids: &'a [Uuid]) -> BoxFut<'a, AppResult<Option<String>>> {
        Box::pin(async { Ok(None) })
    }
    /// Product page shows the seller's contact card (call / WhatsApp) when this extension is present.
    fn show_contact(&self) -> bool {
        false
    }
}

/// Reactions to platform events about a shop. Runs inside the platform's transaction.
pub trait ShopHook: Send + Sync {
    /// Business verification was revoked: take whatever the shop sells off sale.
    fn verification_revoked<'a>(&'a self, conn: &'a mut PgConnection, st: &'a AppState, shop_id: Uuid) -> BoxFut<'a, AppResult<()>>;
}

/// Gross takings (VAT included when the shop charges VAT) of one shop in one currency.
#[derive(Debug, Clone, Serialize)]
pub struct TaxableLine {
    pub shop_id: Uuid,
    pub currency: String,
    pub gross_cents: i64,
    /// Number of sales / orders behind the amount.
    pub count: i64,
}

/// A core's completed sales, for tax filings. Dates: `from` inclusive, `to` exclusive (shop-local = UTC+7).
pub trait TaxSource: Send + Sync {
    /// Stable id, e.g. "commerce.orders".
    fn key(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn taxable<'a>(&'a self, st: &'a AppState, from: NaiveDate, to: NaiveDate, shop: Option<Uuid>) -> BoxFut<'a, AppResult<Vec<TaxableLine>>>;
}

/// What a core crate hands to the gateway.
pub struct CoreModule {
    /// Short id used in env vars (ZK_CORES, ZK_REMOTE_<NAME>) and in /api/cores.
    pub name: &'static str,
    pub title: &'static str,
    pub title_lo: &'static str,
    /// Lucide icon name for the super-app home.
    pub icon: &'static str,
    /// Shop types (`shops.vertical`) this core adds.
    pub verticals: &'static [&'static str],
    /// The core's own migrations (versions must be globally unique: kernel 0001–0999, then 1000s per core).
    pub migrator: Option<&'static Migrator>,
    pub routes: fn(&Config) -> Routes,
    /// Register extension points. Runs whether the core's routes are local or remote (shared database).
    pub install: fn(&mut Registry),
    /// Background jobs; only where the core runs locally.
    pub start: fn(AppState),
    /// Lao translations for this core's error messages (exact, regex patterns).
    pub lao: &'static [(&'static str, &'static str)],
    pub lao_patterns: &'static [(&'static str, &'static str)],
    /// Lao translator function, for cores that keep translations in code.
    pub lao_fn: Option<fn(&str) -> Option<String>>,
    /// Staff permissions of this core's shop routes: (method or "*", route pattern, permission) — see `access`.
    pub access_rules: &'static [(&'static str, &'static str, &'static str)],
}

impl CoreModule {
    pub fn new(name: &'static str, title: &'static str, title_lo: &'static str, icon: &'static str, routes: fn(&Config) -> Routes) -> Self {
        Self {
            name,
            title,
            title_lo,
            icon,
            verticals: &[],
            migrator: None,
            routes,
            install: |_| {},
            start: |_| {},
            lao: &[],
            lao_patterns: &[],
            lao_fn: None,
            access_rules: &[],
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CoreInfo {
    pub name: &'static str,
    pub title: &'static str,
    pub title_lo: &'static str,
    pub icon: &'static str,
    pub verticals: &'static [&'static str],
    /// "local" or the base URL of the instance serving it.
    pub served_by: String,
}

/// Enabled cores and their extension points.
#[derive(Default)]
pub struct Registry {
    pub cores: Vec<CoreInfo>,
    pub listing: Vec<Arc<dyn ListingExt>>,
    pub shop_hooks: Vec<Arc<dyn ShopHook>>,
    pub tax_sources: Vec<Arc<dyn TaxSource>>,
    catalog_ext_sql: String,
}

impl Registry {
    pub fn add_listing(&mut self, ext: impl ListingExt + 'static) {
        self.listing.push(Arc::new(ext));
    }
    pub fn add_shop_hook(&mut self, hook: impl ShopHook + 'static) {
        self.shop_hooks.push(Arc::new(hook));
    }
    pub fn add_tax_source(&mut self, src: impl TaxSource + 'static) {
        self.tax_sources.push(Arc::new(src));
    }
    /// Call once all cores are installed.
    pub fn finish(&mut self) {
        self.catalog_ext_sql = if self.listing.is_empty() {
            "'{}'::jsonb AS ext".into()
        } else {
            let parts: Vec<String> = self.listing.iter().map(|e| format!("'{}', ({})", e.key(), e.card_sql())).collect();
            format!("jsonb_build_object({}) AS ext", parts.join(", "))
        };
    }
    pub fn has_core(&self, name: &str) -> bool {
        self.cores.iter().any(|c| c.name == name)
    }
    /// Shop types offered by the enabled cores.
    pub fn verticals(&self) -> Vec<&'static str> {
        self.cores.iter().flat_map(|c| c.verticals.iter().copied()).collect()
    }
    /// SELECT expression (`… AS ext`) with every listing extension's card summary, keyed by extension.
    pub fn catalog_ext_sql(&self) -> &str {
        &self.catalog_ext_sql
    }
    /// Pick the listing extensions present in a product body (`null` = absent) and validate them.
    pub fn listing_inputs(&self, body: &Map<String, Value>) -> AppResult<Vec<(Arc<dyn ListingExt>, Value)>> {
        let mut out = Vec::new();
        for ext in &self.listing {
            match body.get(ext.key()) {
                None | Some(Value::Null) => {}
                Some(v) => out.push((ext.clone(), ext.clean(v.clone())?)),
            }
        }
        Ok(out)
    }
}
