//! Optional feature modules of the commerce core ("plug-and-play").
//!
//! A module lives in its own folder and owns its routes, background jobs, Lao error messages and
//! one migration file (`migrations/NNNN_<module>.sql`). The core calls only the hooks in this file
//! (plus any documented one-line checkout hooks), so:
//!
//! * **add** a module: drop its folder here, add it to the functions below, add its migration;
//! * **switch it off** at runtime: `<MODULE>_ENABLED=false` — its routes are not mounted (404),
//!   its jobs don't start and its hooks do nothing;
//! * the web app asks `GET /api/modules` which optional screens to show.

use axum::{extract::State, routing::get, Json};
use serde_json::{json, Value};

use crate::AppState;

pub mod cod_risk;
pub mod image_suggest;
pub mod pos_sync;
pub mod promo;

/// Routes of every enabled module (part of the commerce core's routes).
pub fn router() -> kernel::Routes {
    let mut r = kernel::Routes::new().route("/modules", get(status));
    if cod_risk::enabled() {
        r = r.merge(cod_risk::router());
    }
    if image_suggest::enabled() {
        r = r.merge(image_suggest::router());
    }
    if pos_sync::enabled() {
        r = r.merge(pos_sync::router());
    }
    if promo::enabled() {
        r = r.merge(promo::router());
    }
    r
}

/// Background jobs of every enabled module.
pub fn spawn_jobs(state: &AppState) {
    if cod_risk::enabled() {
        cod_risk::spawn_jobs(state.clone());
    }
    if promo::enabled() {
        promo::spawn_jobs(state.clone());
    }
}

/// Lao translations of module error messages (registered with the kernel's i18n by the commerce core).
pub fn translate_lo(msg: &str) -> Option<String> {
    cod_risk::i18n::translate_lo(msg)
        .or_else(|| image_suggest::i18n::translate_lo(msg))
        .or_else(|| pos_sync::i18n::translate_lo(msg))
        .or_else(|| promo::i18n::translate_lo(msg))
}

/// Which optional modules are on (public; no secrets).
async fn status(State(_st): State<AppState>) -> Json<Value> {
    Json(json!({ "cod_risk": cod_risk::public_status(), "image_suggest": image_suggest::public_status(), "pos_sync": pos_sync::public_status(), "promo": promo::public_status() }))
}
