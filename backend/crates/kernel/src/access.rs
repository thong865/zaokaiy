//! Who may act on a shop: the owner, platform admins, and staff through their role's permissions.
//!
//! Every shop-scoped API route maps to exactly one permission ([`required`]). A middleware puts the
//! permission of the matched route into a task-local for the duration of the request, and
//! [`crate::routes::owned_shop`] — which every shop-scoped handler already calls — lets a staff
//! member through only when their role has it. Routes that are not in the table (staff management,
//! business verification, …) stay owner-only, so a new route is safe by default.

use axum::{
    extract::{MatchedPath, Request},
    http::Method,
    middleware::Next,
    response::Response,
};
use serde::Serialize;

/// Permissions a role can grant, in the order the UI shows them.
pub const PERMISSIONS: [&str; 13] = [
    "stats", "products", "inventory", "media", "orders", "delivery", "pos", "pos_void", "social", "leads", "marketing", "network", "settings",
];

/// Lao names of the starter roles (used when the owner's app is in Lao).
pub const TEMPLATE_NAMES_LO: [(&str, &str); 5] = [
    ("manager", "ຜູ້ຈັດການ"),
    ("cashier", "ພະນັກງານເກັບເງິນ"),
    ("stock", "ພະນັກງານສາງ"),
    ("orders", "ຄຳສັ່ງຊື້ ແລະ ຈັດສົ່ງ"),
    ("marketing", "ການຕະຫຼາດ"),
];

/// Starter roles offered to every shop (created the first time the owner opens the staff page).
pub const TEMPLATES: [(&str, &str, &[&str]); 5] = [
    ("manager", "Manager", &["stats", "products", "inventory", "media", "orders", "delivery", "pos", "pos_void", "social", "leads", "marketing", "network", "settings"]),
    ("cashier", "Cashier", &["pos"]),
    ("stock", "Stock keeper", &["products", "inventory", "media"]),
    ("orders", "Orders & delivery", &["orders", "delivery", "social", "leads"]),
    ("marketing", "Marketing", &["marketing", "media", "products", "stats"]),
];

/// (method or "*", route pattern, permission). A pattern ending in `*` matches by prefix.
/// First match wins, so specific rules come before broader ones.
const RULES: &[(&str, &str, &str)] = &[
    ("PATCH", "/shops/{id}", "settings"),
    ("*", "/shops/{id}/stats", "stats"),
    ("*", "/shops/{id}/products", "products"),
    ("*", "/products/{id}/stock", "inventory"),
    ("*", "/products/{id}*", "products"),
    ("*", "/shops/{id}/categories", "products"),
    ("*", "/categories/*", "products"),
    ("*", "/shops/{id}/barcodes/*", "products"),
    ("*", "/shops/{id}/inventory/*", "inventory"),
    ("*", "/shops/{id}/loyalty/lookup", "pos"),
    ("*", "/shops/{id}/loyalty/quote", "pos"),
    ("*", "/shops/{id}/loyalty*", "marketing"),
    ("*", "/shops/{id}/promo/*", "marketing"),
    ("*", "/promo/campaigns/*", "marketing"),
    ("*", "/shops/{id}/image-suggest/sharing", "media"),
    ("*", "/shops/{id}/image-suggest*", "products"),
    ("*", "/shops/{id}/media*", "media"),
    ("*", "/media/*", "media"),
    ("*", "/shops/{id}/sales", "orders"),
    ("*", "/shops/{id}/orders/*", "orders"),
    ("GET", "/orders/{id}", "orders"),
    ("GET", "/orders/{id}/document", "orders"),
    ("*", "/shops/{id}/shipping*", "delivery"),
    ("*", "/shops/{id}/cod*", "delivery"),
    ("*", "/cod-risk/*", "delivery"),
    ("POST", "/pos/sales/{id}/void", "pos_void"),
    ("*", "/shops/{id}/pos/*", "pos"),
    ("*", "/pos/*", "pos"),
    ("*", "/shops/{id}/social/*", "social"),
    ("*", "/social/*", "social"),
    ("*", "/shops/{id}/leads", "leads"),
    ("*", "/leads/*", "leads"),
    ("*", "/shops/{id}/ads", "marketing"),
    ("PATCH", "/ads/{id}", "marketing"),
    ("*", "/shops/{id}/contents", "marketing"),
    ("*", "/contents/*", "marketing"),
    ("*", "/shops/{id}/ai/*", "marketing"),
    ("*", "/shops/{id}/partnerships", "network"),
    ("*", "/partnerships/*", "network"),
    ("*", "/shops/{id}/listings", "network"),
    ("*", "/listings/*", "network"),
    ("*", "/shops/{id}/commissions*", "network"),
    ("*", "/commissions/*", "network"),
];

/// Rules added by cores at startup (restaurant, insurance …), checked after the built-in ones.
static EXTRA: std::sync::RwLock<Vec<(&'static str, &'static str, &'static str)>> = std::sync::RwLock::new(Vec::new());

/// Add a core's (method or "*", route pattern, permission) rules. Permissions must be in [`PERMISSIONS`].
pub fn register_rules(rules: &'static [(&'static str, &'static str, &'static str)]) {
    assert!(rules.iter().all(|(_, _, p)| PERMISSIONS.contains(p)), "unknown permission in access rules");
    EXTRA.write().expect("access lock").extend_from_slice(rules);
}

/// The permission a staff member needs for this route; None = owner only.
pub fn required(method: &Method, path: &str) -> Option<&'static str> {
    let path = path.strip_prefix("/api").unwrap_or(path);
    let extra = EXTRA.read().expect("access lock");
    RULES
        .iter()
        .chain(extra.iter())
        .find(|(m, pat, _)| {
            (*m == "*" || *m == method.as_str())
                && match pat.strip_suffix('*') {
                    Some(prefix) => path.starts_with(prefix),
                    None => path == *pat,
                }
        })
        .map(|(_, _, p)| *p)
}

tokio::task_local! {
    static REQUIRED: Option<&'static str>;
}

/// Route layer: remember which permission the matched route needs.
pub async fn scope_permission(req: Request, next: Next) -> Response {
    let perm = req.extensions().get::<MatchedPath>().and_then(|m| required(req.method(), m.as_str()));
    REQUIRED.scope(perm, next.run(req)).await
}

/// Permission required by the request being handled (None outside a request or for owner-only routes).
pub fn current() -> Option<&'static str> {
    REQUIRED.try_with(|p| *p).ok().flatten()
}

pub fn valid(perms: &[String]) -> bool {
    perms.iter().all(|p| PERMISSIONS.contains(&p.as_str()))
}

/// How the caller relates to a shop (sent to the web app with /me/shops).
#[derive(Debug, Clone, Serialize)]
pub struct Access {
    pub owner: bool,
    pub role_id: Option<uuid::Uuid>,
    pub role: String,
    pub permissions: Vec<String>,
}

impl Access {
    pub fn owner() -> Self {
        Self { owner: true, role_id: None, role: "owner".into(), permissions: PERMISSIONS.iter().map(|s| s.to_string()).collect() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_map_to_permissions() {
        let g = Method::GET;
        let p = Method::POST;
        let cases: &[(&Method, &str, Option<&str>)] = &[
            (&Method::PATCH, "/api/shops/{id}", Some("settings")),
            (&g, "/shops/{id}", None),
            (&g, "/shops/{id}/products", Some("products")),
            (&p, "/products/{id}/stock", Some("inventory")),
            (&Method::PUT, "/products/{id}/media", Some("products")),
            (&g, "/products/{id}/vehicle", Some("products")),
            (&p, "/shops/{id}/pos/sales", Some("pos")),
            (&p, "/pos/sales/{id}/void", Some("pos_void")),
            (&g, "/pos/sales/{id}", Some("pos")),
            (&Method::PUT, "/pos/bills/{id}", Some("pos")),
            (&g, "/shops/{id}/cod", Some("delivery")),
            (&g, "/shops/{id}/cod-risk/unshipped", Some("delivery")),
            (&g, "/shops/{id}/image-suggest", Some("products")),
            (&Method::PUT, "/shops/{id}/image-suggest/sharing", Some("media")),
            (&p, "/shops/{id}/ai/agent", Some("marketing")),
            (&g, "/shops/{id}/loyalty/lookup", Some("pos")),
            (&p, "/shops/{id}/loyalty/members/{member}/adjust", Some("marketing")),
            (&g, "/shops/{id}/promo/campaigns", Some("marketing")),
            (&Method::PATCH, "/promo/campaigns/{id}", Some("marketing")),
            (&p, "/commissions/{id}/pay", Some("network")),
            (&g, "/orders/{id}", Some("orders")),
            (&p, "/orders/{id}/cancel", None),
            // owner only
            (&g, "/shops/{id}/kyb", None),
            (&p, "/shops/{id}/staff/invites", None),
            (&g, "/shops/{id}/staff", None),
        ];
        for (m, path, want) in cases {
            assert_eq!(required(m, path), *want, "{m} {path}");
        }
    }

    #[test]
    fn templates_use_known_permissions() {
        for (_, _, perms) in TEMPLATES {
            assert!(perms.iter().all(|p| PERMISSIONS.contains(p)));
        }
        assert!(valid(&["pos".into()]) && !valid(&["staff".into()]));
    }
}
