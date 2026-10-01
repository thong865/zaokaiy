//! Copying photo files between the stock library and shop libraries, and storing uploads.

use bytes::Bytes;
use serde_json::{json, Value};

use crate::{
    error::{AppError, AppResult},
    media, AppState,
};

/// The file side of a photo (a stock image or a media asset).
#[derive(Debug, Clone, Default)]
pub struct Files {
    pub mime: String,
    pub url: String,
    pub thumb_url: Option<String>,
    pub storage_key: Option<String>,
    pub thumb_key: Option<String>,
    pub size_bytes: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub variants: Value,
    pub variant_keys: Vec<String>,
    pub placeholder: String,
    pub dominant_color: String,
}

impl Files {
    pub fn keys(&self) -> Vec<String> {
        [&self.storage_key, &self.thumb_key].into_iter().flatten().cloned().chain(self.variant_keys.iter().cloned()).collect()
    }
}

fn ext_of(key: &str) -> &str {
    key.rsplit_once('.').map(|(_, e)| e).filter(|e| e.len() <= 5).unwrap_or("bin")
}

fn mime_of(ext: &str) -> &'static str {
    match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

pub async fn delete_keys(st: &AppState, keys: &[String]) {
    for k in keys {
        st.storage.delete(k).await;
    }
}

/// Copy every stored file of `src` to new keys under `base` (e.g. `shops/<id>/2026/09/<uuid>`).
/// External-URL photos (no storage key) are copied by reference. On error nothing is left behind.
pub async fn copy_files(st: &AppState, src: &Files, base: &str) -> AppResult<Files> {
    let mut done: Vec<String> = Vec::new();
    let r = copy_inner(st, src, base, &mut done).await;
    if r.is_err() {
        delete_keys(st, &done).await;
    }
    r
}

async fn copy_inner(st: &AppState, src: &Files, base: &str, done: &mut Vec<String>) -> AppResult<Files> {
    let mut out = src.clone();
    let Some(main) = &src.storage_key else {
        return Ok(out);
    };
    let mut cp = |from: String, to: String| {
        done.push(to.clone());
        async move {
            let data = st.storage.get(&from).await?;
            st.storage.put(&to, data, mime_of(ext_of(&to))).await?;
            Ok::<String, AppError>(st.storage.url(&to))
        }
    };
    let key = format!("{base}.{}", ext_of(main));
    out.url = cp(main.clone(), key.clone()).await?;
    out.storage_key = Some(key);
    if let Some(t) = &src.thumb_key {
        let k = format!("{base}_thumb.{}", ext_of(t));
        out.thumb_url = Some(cp(t.clone(), k.clone()).await?);
        out.thumb_key = Some(k);
    }
    // Renditions: keys and the public list are in the same order (largest first).
    let list = src.variants.as_array().cloned().unwrap_or_default();
    let mut variants = Vec::new();
    let mut keys = Vec::new();
    for (i, vk) in src.variant_keys.iter().enumerate() {
        let w = list.get(i).and_then(|v| v["w"].as_u64()).unwrap_or(i as u64);
        let k = format!("{base}_w{w}.webp");
        let url = cp(vk.clone(), k.clone()).await?;
        variants.push(json!({ "w": list.get(i).map(|v| v["w"].clone()).unwrap_or(json!(w)), "h": list.get(i).map(|v| v["h"].clone()).unwrap_or(Value::Null), "url": url }));
        keys.push(k);
    }
    out.variants = Value::Array(variants);
    out.variant_keys = keys;
    Ok(out)
}

/// Process and store an uploaded photo under `base` (same pipeline as the media library:
/// orientation, resize, EXIF stripped, WebP renditions, placeholder, colour).
pub async fn store_upload(st: &AppState, base: &str, data: Bytes) -> AppResult<Files> {
    let max_image = st.cfg.media_max_image_mb * 1_048_576;
    let min_edge = st.cfg.media_min_image_edge;
    let permit = media::cpu_permit().await;
    let p = tokio::task::spawn_blocking(move || media::process(data, max_image, 0, min_edge))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    drop(permit);
    if p.kind != "image" {
        return Err(AppError::bad("stock photos must be images (JPG, PNG, WebP)"));
    }
    let mut done: Vec<String> = Vec::new();
    let r: AppResult<Files> = async {
        let key = format!("{base}.{}", p.ext);
        done.push(key.clone());
        st.storage.put(&key, p.main.clone(), &p.mime).await?;
        let thumb_key = match &p.thumb {
            Some(t) => {
                let k = format!("{base}_thumb.{}", p.thumb_ext);
                done.push(k.clone());
                st.storage.put(&k, t.clone(), p.thumb_mime).await?;
                Some(k)
            }
            None => None,
        };
        let mut variants = Vec::new();
        let mut keys = Vec::new();
        for v in &p.variants {
            let k = format!("{base}_w{}.webp", v.width);
            done.push(k.clone());
            st.storage.put(&k, v.data.clone(), "image/webp").await?;
            variants.push(json!({ "w": v.width, "h": v.height, "url": st.storage.url(&k) }));
            keys.push(k);
        }
        Ok(Files {
            mime: p.mime.clone(),
            url: st.storage.url(&key),
            thumb_url: thumb_key.as_deref().map(|k| st.storage.url(k)),
            storage_key: Some(key),
            thumb_key,
            size_bytes: p.main.len() as i64 + p.thumb.as_ref().map(|t| t.len() as i64).unwrap_or(0) + p.variants.iter().map(|v| v.data.len() as i64).sum::<i64>(),
            width: p.width,
            height: p.height,
            variants: Value::Array(variants),
            variant_keys: keys,
            placeholder: p.placeholder.clone(),
            dominant_color: p.color.clone(),
        })
    }
    .await;
    if r.is_err() {
        delete_keys(st, &done).await;
    }
    r
}
