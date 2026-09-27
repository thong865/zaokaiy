//! Object storage for uploaded media.
//!
//! * `local` (default): files on disk under `MEDIA_DIR`, served by the API at `/media/*`.
//! * `s3`: any S3-compatible store (AWS S3, MinIO, Cloudflare R2, Wasabi…). Objects are
//!   written with their content type; `MEDIA_PUBLIC_URL` should point at the bucket or a CDN.

use std::{path::PathBuf, sync::Arc};

use bytes::Bytes;
use object_store::{
    aws::AmazonS3Builder, path::Path as ObjPath, Attribute, Attributes, ObjectStore, PutOptions,
    PutPayload,
};

use crate::{config::Config, error::AppError};

#[derive(Clone)]
pub enum Storage {
    Local { dir: PathBuf, public_base: String },
    S3 { store: Arc<dyn ObjectStore>, public_base: String },
}

impl Storage {
    pub fn from_config(cfg: &Config) -> Self {
        let public_base = cfg.media_public_url.trim_end_matches('/').to_string();
        match cfg.media_driver.as_str() {
            "s3" => {
                let mut b = AmazonS3Builder::new()
                    .with_bucket_name(&cfg.s3_bucket)
                    .with_region(&cfg.s3_region)
                    .with_access_key_id(&cfg.s3_access_key)
                    .with_secret_access_key(&cfg.s3_secret_key);
                if !cfg.s3_endpoint.is_empty() {
                    b = b
                        .with_endpoint(&cfg.s3_endpoint)
                        .with_virtual_hosted_style_request(false)
                        .with_allow_http(cfg.s3_endpoint.starts_with("http://"));
                }
                let store = b.build().expect("configure S3 media storage");
                tracing::info!(bucket = %cfg.s3_bucket, "media storage: s3");
                Storage::S3 { store: Arc::new(store), public_base }
            }
            _ => {
                let dir = PathBuf::from(&cfg.media_dir);
                std::fs::create_dir_all(&dir).expect("create MEDIA_DIR");
                tracing::info!(dir = %dir.display(), "media storage: local");
                Storage::Local { dir, public_base }
            }
        }
    }

    pub fn url(&self, key: &str) -> String {
        match self {
            Storage::Local { public_base, .. } | Storage::S3 { public_base, .. } => {
                format!("{public_base}/{key}")
            }
        }
    }

    pub fn local_dir(&self) -> Option<&PathBuf> {
        match self {
            Storage::Local { dir, .. } => Some(dir),
            _ => None,
        }
    }

    pub async fn put(&self, key: &str, data: Bytes, content_type: &str) -> Result<(), AppError> {
        match self {
            Storage::Local { dir, .. } => {
                let path = dir.join(key);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| AppError::Internal(format!("mkdir: {e}")))?;
                }
                tokio::fs::write(&path, &data)
                    .await
                    .map_err(|e| AppError::Internal(format!("write media: {e}")))
            }
            Storage::S3 { store, .. } => {
                let mut attributes = Attributes::new();
                attributes.insert(Attribute::ContentType, content_type.to_string().into());
                attributes.insert(
                    Attribute::CacheControl,
                    "public, max-age=31536000, immutable".to_string().into(),
                );
                store
                    .put_opts(
                        &ObjPath::from(key),
                        PutPayload::from(data),
                        PutOptions { attributes, ..Default::default() },
                    )
                    .await
                    .map(|_| ())
                    .map_err(|e| AppError::Upstream(format!("media upload failed: {e}")))
            }
        }
    }

    /// Best-effort delete; missing objects are ignored.
    pub async fn delete(&self, key: &str) {
        let res = match self {
            Storage::Local { dir, .. } => tokio::fs::remove_file(dir.join(key))
                .await
                .map_err(|e| e.to_string()),
            Storage::S3 { store, .. } => store
                .delete(&ObjPath::from(key))
                .await
                .map_err(|e| e.to_string()),
        };
        if let Err(e) = res {
            tracing::debug!(%key, error = %e, "media delete (ignored)");
        }
    }
}
