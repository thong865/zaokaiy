//! Background sync: upload the outbox, then pull catalogue changes. Runs every few seconds and
//! right after each sale; any failure just leaves everything queued for the next round.

use std::{sync::Arc, time::Duration};

use serde::Serialize;
use serde_json::Value;
use tokio::sync::{Mutex as AsyncMutex, Notify};

use crate::{
    api::{Api, ApiError},
    db::{now, PullPage, PushResult, Store},
};

const BATCH: i64 = 100;
const ONLINE_EVERY: Duration = Duration::from_secs(30);
const OFFLINE_EVERY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct SyncStatus {
    pub paired: bool,
    pub online: bool,
    pub syncing: bool,
    /// ok | unpaired (token revoked) | forbidden (cashier lost POS access)
    pub auth: String,
    pub pending: i64,
    pub rejected: i64,
    pub products: i64,
    pub last_sync_at: Option<String>,
    pub last_error: Option<String>,
}

pub struct Engine {
    pub store: Arc<Store>,
    status: std::sync::Mutex<SyncStatus>,
    wake: Notify,
    running: AsyncMutex<()>,
}

impl Engine {
    pub fn new(store: Arc<Store>) -> Arc<Self> {
        let e = Arc::new(Self { store, status: Default::default(), wake: Notify::new(), running: AsyncMutex::new(()) });
        let mut s = e.status.lock().unwrap();
        s.auth = "ok".into();
        s.last_sync_at = e.store.get("last_sync_at");
        drop(s);
        e
    }

    /// API client for the paired shop (None before pairing).
    pub fn api(&self) -> Option<Api> {
        let base = self.store.get("api_base")?;
        let token = self.store.get("token")?;
        Some(Api::new(&base, Some(token), &self.store.get("lang").unwrap_or_else(|| "en".into())))
    }

    fn shop_id(&self) -> Option<String> {
        self.store.get_json("shop")["id"].as_str().map(String::from)
    }

    /// Current status with fresh queue counts.
    pub fn status(&self) -> SyncStatus {
        let mut s = self.status.lock().unwrap().clone();
        s.paired = self.store.get("token").is_some();
        let (p, r) = self.shop_id().map(|id| self.store.queue_counts(&id)).unwrap_or((0, 0));
        s.pending = p;
        s.rejected = r;
        s.products = self.store.product_count();
        s
    }

    fn update(&self, f: impl FnOnce(&mut SyncStatus)) {
        f(&mut self.status.lock().unwrap());
    }

    /// Ask the loop to sync now (after a sale, on "sync now", when the network comes back).
    pub fn kick(&self) {
        self.wake.notify_one();
    }

    /// One round: push everything queued, then pull until up to date.
    pub async fn cycle(&self) -> SyncStatus {
        let Ok(_guard) = self.running.try_lock() else {
            return self.status(); // a round is already running
        };
        let Some(api) = self.api() else {
            self.update(|s| {
                s.online = false;
                s.syncing = false;
            });
            return self.status();
        };
        self.update(|s| s.syncing = true);
        let result = self.round(&api).await;
        self.update(|s| {
            s.syncing = false;
            match &result {
                Ok(()) => {
                    s.online = true;
                    s.auth = "ok".into();
                    s.last_error = None;
                    let at = now();
                    let _ = self.store.set("last_sync_at", &at);
                    s.last_sync_at = Some(at);
                }
                Err(e) => {
                    s.online = !e.is_offline();
                    s.last_error = if e.is_offline() { None } else { Some(e.to_string()) };
                    s.auth = match e {
                        ApiError::Unauthorized => "unpaired".into(),
                        ApiError::Forbidden => "forbidden".into(),
                        _ => s.auth.clone(),
                    };
                }
            }
        });
        self.status()
    }

    async fn round(&self, api: &Api) -> Result<(), ApiError> {
        if let Some(shop) = self.shop_id() {
            self.push_all(api, &shop).await?;
        } else {
            // Freshly paired: learn the shop and terminal first.
            let me = api.me().await?;
            let _ = self.store.set_json("shop", &me["shop"]);
            let _ = self.store.set_json("device", &me["device"]);
        }
        self.pull_all(api).await
    }

    async fn push_all(&self, api: &Api, shop: &str) -> Result<(), ApiError> {
        for _ in 0..50 {
            let (sales, voids) = self.store.outbox(shop, BATCH).map_err(|e| ApiError::Http(0, e))?;
            if sales.is_empty() && voids.is_empty() {
                break;
            }
            let res = api.push(&sales, &voids).await?;
            let parse = |k: &str| -> Vec<PushResult> { serde_json::from_value(res[k].clone()).unwrap_or_default() };
            self.store.apply_push_results(&parse("sales"), &parse("voids")).map_err(|e| ApiError::Http(0, e))?;
            if (sales.len() as i64) < BATCH && (voids.len() as i64) < BATCH {
                break;
            }
        }
        Ok(())
    }

    async fn pull_all(&self, api: &Api) -> Result<(), ApiError> {
        let mut cursor = self.store.get("cursor").unwrap_or_default();
        for _ in 0..10_000 {
            let v: Value = api.pull(&cursor).await?;
            let page: PullPage = serde_json::from_value(v).map_err(|e| ApiError::Http(0, format!("unexpected sync data: {e}")))?;
            self.store.apply_pull(&page).map_err(|e| ApiError::Http(0, e))?;
            cursor = page.cursor.clone();
            if !page.has_more {
                break;
            }
        }
        Ok(())
    }

    /// Forever: sync, report, wait for a kick or the next tick (sooner when offline, so the till
    /// notices the network coming back quickly).
    pub async fn run(self: Arc<Self>, on_status: impl Fn(SyncStatus) + Send + 'static) {
        let mut last = SyncStatus::default();
        loop {
            let s = self.cycle().await;
            if s != last {
                on_status(s.clone());
                last = s.clone();
            }
            let wait = if s.online { ONLINE_EVERY } else { OFFLINE_EVERY };
            tokio::select! {
                _ = self.wake.notified() => {}
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }
}
