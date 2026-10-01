//! Platform core: identity (email / Google / Facebook / WhatsApp login, 2FA, captcha), shops, staff and corporate KYC.
//! Every other core depends on the users and shops it owns.

pub use kernel::{access, auth, config, crypto, error, guards, models, AppState};

/// Ownership checks (kept under `routes::` like the other cores).
pub mod routes {
    pub use kernel::guards::{owned_shop, owner_shop};
}

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, patch, post},
};
use kernel::{config::Config, CoreModule, Routes};

pub mod auth_routes;
pub mod captcha;
pub mod kyb;
pub mod oauth;
pub mod shops;
pub mod staff;
pub mod totp;
pub mod two_factor;

pub fn module() -> CoreModule {
    CoreModule::new("platform", "Accounts & shops", "ບັນຊີ ແລະ ຮ້ານຄ້າ", "user-round", routes)
}

fn routes(_cfg: &Config) -> Routes {
    Routes::new()
        // auth
        .route("/auth/register", post(auth_routes::register))
        .route("/auth/login", post(auth_routes::login))
        .route("/auth/me", get(auth_routes::me))
        // social login (Google, Facebook, WhatsApp) + connected accounts
        .route("/auth/providers", get(oauth::providers))
        .route("/auth/oauth/{provider}/start", post(oauth::start))
        .route("/auth/oauth/{provider}/callback", get(oauth::callback))
        .route("/auth/exchange", post(oauth::exchange))
        .route("/auth/whatsapp/send", post(oauth::whatsapp_send))
        .route("/auth/whatsapp/verify", post(oauth::whatsapp_verify))
        .route("/auth/identities", get(oauth::identities))
        .route("/auth/identities/{provider}", delete(oauth::unlink))
        .route("/auth/password", post(oauth::set_password))
        // two-factor authentication
        .route("/auth/2fa", get(two_factor::status))
        .route("/auth/2fa/setup", post(two_factor::setup))
        .route("/auth/2fa/enable", post(two_factor::enable))
        .route("/auth/2fa/disable", post(two_factor::disable))
        .route("/auth/2fa/recovery-codes", post(two_factor::regenerate))
        .route("/auth/2fa/verify", post(two_factor::verify))
        // shops
        .route("/me/shops", get(shops::my_shops))
        .route("/shops", post(shops::create_shop))
        .route("/shops/{id}", patch(shops::update_shop))
        // corporate KYC (business verification)
        .route("/shops/{id}/kyb", get(kyb::get_kyb).put(kyb::save_kyb))
        .route("/shops/{id}/kyb/submit", post(kyb::submit))
        .route("/shops/{id}/kyb/documents", post(kyb::upload_document).layer(DefaultBodyLimit::max(12 * 1_048_576)))
        .route("/kyb/documents/{id}", delete(kyb::delete_document))
        .route("/kyb/documents/{id}/file", get(kyb::document_file))
        .route("/admin/kyb", get(kyb::admin_queue))
        .route("/admin/kyb/{shop_id}", get(kyb::admin_detail))
        .route("/admin/kyb/{shop_id}/decision", post(kyb::admin_decide))
        // shop staff: roles, invites, members (owner only) + joining a shop
        .route("/shops/{id}/staff", get(staff::overview))
        .route("/shops/{id}/staff/roles", post(staff::create_role))
        .route("/shop-roles/{id}", patch(staff::update_role).delete(staff::delete_role))
        .route("/shops/{id}/staff/invites", post(staff::create_invite))
        .route("/staff-invites/{id}", delete(staff::revoke_invite))
        .route("/shops/{id}/staff/{user_id}", patch(staff::update_member).delete(staff::remove_member))
        .route("/join/{token}", get(staff::invite_info).post(staff::accept_invite))
        .route("/me/memberships/{shop_id}", delete(staff::leave))
}
