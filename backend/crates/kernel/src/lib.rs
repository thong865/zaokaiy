//! Shared kernel for every zaokaiy core: configuration, errors (+ Lao translations), JWT auth,
//! encryption, file storage, image processing, shared models and the core registry.
//!
//! A *core* (commerce, vehicle, restaurant, …) is a crate exposing `pub fn module() -> CoreModule`.
//! The gateway collects the enabled modules, builds one [`AppState`] and mounts each core's routes
//! locally — or proxies them to another instance running that core (`ZK_REMOTE_<CORE>`).

pub mod access;
pub mod auth;
pub mod config;
pub mod core;
pub mod crypto;
pub mod error;
pub mod guards;
pub mod i18n;
pub mod media;
pub mod models;
pub mod storage;
pub mod util;

use std::sync::Arc;

pub use core::{BoxFut, CoreModule, Registry, Routes};

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub cfg: Arc<config::Config>,
    pub http: reqwest::Client,
    pub storage: storage::Storage,
    /// Private storage for KYC documents (never served publicly).
    pub private: storage::Storage,
    /// Encryption for KYC documents and identity / bank numbers.
    pub sealer: crypto::Sealer,
    /// Enabled cores and the extension points they registered.
    pub reg: Arc<Registry>,
}

/// Baseline schema shared by the platform, commerce and vehicle cores (versions 0001–0999).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
