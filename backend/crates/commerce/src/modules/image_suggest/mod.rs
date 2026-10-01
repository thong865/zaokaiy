//! # Image suggestions — reuse photos that are already on the platform
//!
//! While a seller types a product name ("beer lao", "ເບຍລາວ"), the product form shows matching
//! photos they can add with one click:
//!
//! 1. **Stock library** — photos curated by admins (brand shots, common items) with a title,
//!    keywords in any language, brand and credit. Admins upload them or promote a shop's photo.
//! 2. **The seller's own library** — earlier uploads matching the name.
//! 3. **Shared shop photos** — live, approved product photos of shops that opted in
//!    ("let other sellers use my product photos"). Off per shop by default;
//!    `IMAGE_SUGGEST_SHARED=false` turns this source off platform-wide.
//!
//! Matching works without spaces (Lao/Thai): compact substring tests + admin synonym groups
//! ("beerlao" = "ເບຍລາວ" = "เบียร์ลาว") + character-bigram similarity, see [`text`].
//! Picking a stock/shared photo **copies** its files into the seller's library, so it behaves like
//! their own upload (alt text, gallery, delete) and never depends on the original.

use std::sync::LazyLock;

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, patch, post},
};
use serde_json::{json, Value};


mod admin;
pub mod i18n;
mod seller;
pub mod store;
pub mod text;

#[derive(Debug, Clone)]
pub struct Config {
    pub enabled: bool,
    /// Suggest photos of shops that opted in to sharing.
    pub shared: bool,
}

static CFG: LazyLock<Config> = LazyLock::new(|| {
    let get = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string()).trim().to_lowercase();
    Config { enabled: get("IMAGE_SUGGEST_ENABLED", "true") != "false", shared: get("IMAGE_SUGGEST_SHARED", "true") != "false" }
});

pub fn cfg() -> &'static Config {
    &CFG
}

pub fn enabled() -> bool {
    cfg().enabled
}

pub fn public_status() -> Value {
    json!({ "enabled": cfg().enabled, "shared": cfg().enabled && cfg().shared })
}

pub fn router() -> kernel::Routes {
    kernel::Routes::new()
        .route("/shops/{id}/image-suggest", get(seller::suggest))
        .route("/shops/{id}/image-suggest/pick", post(seller::pick))
        .route("/shops/{id}/image-suggest/sharing", get(seller::get_sharing).put(seller::set_sharing))
        .route("/admin/image-suggest/stock", get(admin::list_stock).post(admin::upload_stock).layer(DefaultBodyLimit::max(32 * 1_048_576)))
        .route("/admin/image-suggest/stock/promote", post(admin::promote))
        .route("/admin/image-suggest/stock/{id}", patch(admin::update_stock).delete(admin::delete_stock))
        .route("/admin/image-suggest/candidates", get(admin::candidates))
        .route("/admin/image-suggest/synonyms", get(admin::synonyms).post(admin::add_synonym))
        .route("/admin/image-suggest/synonyms/{id}", delete(admin::delete_synonym))
}
