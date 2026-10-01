//! # POS sync — desktop terminals that keep selling offline
//!
//! The zaokaiy POS desktop app (Windows / macOS, `pos-desktop/`) keeps a copy of the shop's
//! catalogue in a local SQLite database, records every sale there first, and syncs with this module
//! whenever it is online. A terminal never needs the network to ring up, print or void a sale.
//!
//! * **Pairing** (device-authorisation flow). The app asks for a code (`POST /pos/pair/start`),
//!   shows it, and polls. A signed-in owner or cashier opens `/pos-pair?code=…` on the web, picks
//!   the shop and approves. The next poll returns a long-lived **device token** (`zkd_…`, only its
//!   hash is stored). The terminal acts as the person who approved it; owners see and revoke
//!   terminals at `/dashboard/pos-devices`. This avoids passwords, captcha and 2FA on the till.
//! * **Receipt numbers.** Every terminal has a short code (T01, T02 …) and numbers its own receipts
//!   without gaps: `<receipt_prefix><code>-000001`. Numbers printed offline are final.
//! * **Pull** (`GET /pos/device/pull?cursor=`): products changed since the cursor (transaction-id
//!   cursor, so a long transaction that commits late is never skipped), deleted product ids, and
//!   the shop's receipt/VAT settings and categories. Paged; `has_more` means "call again".
//! * **Push** (`POST /pos/device/push`): sales and voids made on the terminal. The sale id comes from
//!   the terminal, so re-sending after a timeout is harmless. Totals are re-computed and must match.
//!   A sale that happened offline is always accepted even if the server's stock ran out meanwhile:
//!   stock goes down to zero and the missing units are recorded in `stock_shortfall`.
//!
//! Switch off with `POS_SYNC_ENABLED=false`.

use axum::{
    routing::{get, post},
};
use serde_json::{json, Value};


mod device;
pub mod i18n;
mod pairing;
mod sync;

pub fn enabled() -> bool {
    std::env::var("POS_SYNC_ENABLED").map(|v| v.trim().to_lowercase() != "false").unwrap_or(true)
}

pub fn public_status() -> Value {
    json!({ "enabled": enabled() })
}

pub fn router() -> kernel::Routes {
    kernel::Routes::new()
        // pairing (terminal side: start + poll are public; the web approves)
        .route("/pos/pair/start", post(pairing::start))
        .route("/pos/pair/poll", post(pairing::poll))
        .route("/pos/pair/approve", post(pairing::approve))
        .route("/pos/pair/{code}", get(pairing::info))
        // owner: terminals of a shop
        .route("/shops/{id}/pos-devices", get(pairing::list_devices))
        .route("/shops/{id}/pos-devices/{device_id}/revoke", post(pairing::revoke))
        // terminal (device token)
        .route("/pos/device/me", get(sync::me))
        .route("/pos/device/pull", get(sync::pull))
        .route("/pos/device/push", post(sync::push))
}

pub(crate) fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(s.as_bytes()))
}
