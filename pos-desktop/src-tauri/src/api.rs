//! HTTP client for the zaokaiy API (module `pos_sync`). Runs in Rust, not in the webview, so the
//! terminal needs no CORS setup and the device token never reaches page JavaScript.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// No connection / DNS / timeout: the terminal is offline.
    #[error("offline: {0}")]
    Offline(String),
    /// Device token unknown or revoked: pair again.
    #[error("this terminal is no longer paired")]
    Unauthorized,
    /// The terminal's cashier lost POS access to the shop.
    #[error("the cashier of this terminal no longer has POS access")]
    Forbidden,
    #[error("{1}")]
    Http(u16, String),
}

impl ApiError {
    pub fn is_offline(&self) -> bool {
        matches!(self, ApiError::Offline(_))
    }
}

#[derive(Clone)]
pub struct Api {
    http: reqwest::Client,
    pub base: String,
    token: Option<String>,
    lang: String,
}

impl Api {
    pub fn new(base: &str, token: Option<String>, lang: &str) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(6))
            .timeout(Duration::from_secs(45))
            .user_agent(concat!("zaokaiy-pos/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        Self { http, base: base.trim_end_matches('/').to_string(), token, lang: lang.to_string() }
    }

    async fn send<T: DeserializeOwned>(&self, req: reqwest::RequestBuilder) -> Result<T, ApiError> {
        let mut req = req.header("Accept-Language", &self.lang);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let res = req.send().await.map_err(|e| ApiError::Offline(e.to_string()))?;
        let status = res.status().as_u16();
        let body = res.bytes().await.map_err(|e| ApiError::Offline(e.to_string()))?;
        match status {
            200..=299 => serde_json::from_slice(&body).map_err(|e| ApiError::Http(status, format!("unexpected response: {e}"))),
            401 => Err(ApiError::Unauthorized),
            403 => Err(ApiError::Forbidden),
            // Proxies answer 502/503/504 while the API is down: treat like offline and retry later.
            502..=504 => Err(ApiError::Offline(format!("server unavailable ({status})"))),
            _ => {
                let msg = serde_json::from_slice::<Value>(&body)
                    .ok()
                    .and_then(|v| v["message"].as_str().or(v["error"].as_str()).map(String::from))
                    .unwrap_or_else(|| format!("HTTP {status}"));
                Err(ApiError::Http(status, msg))
            }
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub async fn health(&self) -> Result<Value, ApiError> {
        self.send(self.http.get(self.url("/health")).timeout(Duration::from_secs(8))).await
    }

    pub async fn pair_start(&self, name: &str, install_id: &str) -> Result<Value, ApiError> {
        let body = json!({ "name": name, "platform": std::env::consts::OS, "app_version": env!("CARGO_PKG_VERSION"), "install_id": install_id });
        self.send(self.http.post(self.url("/pos/pair/start")).json(&body)).await
    }

    pub async fn pair_poll(&self, pair_id: &str, secret: &str) -> Result<Value, ApiError> {
        self.send(self.http.post(self.url("/pos/pair/poll")).json(&json!({ "pair_id": pair_id, "poll_secret": secret }))).await
    }

    pub async fn me(&self) -> Result<Value, ApiError> {
        self.send(self.http.get(self.url("/pos/device/me"))).await
    }

    pub async fn pull(&self, cursor: &str) -> Result<Value, ApiError> {
        self.send(self.http.get(self.url("/pos/device/pull")).query(&[("cursor", cursor)])).await
    }

    pub async fn push(&self, sales: &[Value], voids: &[Value]) -> Result<Value, ApiError> {
        self.send(self.http.post(self.url("/pos/device/push")).json(&json!({ "sales": sales, "voids": voids }))).await
    }
}
