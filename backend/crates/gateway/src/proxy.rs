//! Streaming reverse proxy for cores that run in another instance.

use axum::{
    body::Body,
    extract::{OriginalUri, Request},
    http::{header, HeaderMap, HeaderName, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::TryStreamExt;
use kernel::error::AppError;

#[derive(Clone)]
pub struct Target {
    base: String,
    http: reqwest::Client,
}

impl Target {
    pub fn new(base: String, http: reqwest::Client) -> Self {
        Self { base, http }
    }
}

/// Hop-by-hop headers are not forwarded (RFC 9110 §7.6.1); length is recomputed.
fn hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection" | "keep-alive" | "proxy-authenticate" | "proxy-authorization" | "te" | "trailer" | "transfer-encoding" | "upgrade" | "host" | "content-length"
    )
}

fn copy_headers(from: &HeaderMap, to: &mut HeaderMap) {
    for (k, v) in from {
        if !hop_by_hop(k) {
            to.append(k.clone(), v.clone());
        }
    }
}

pub async fn forward(t: Target, req: Request) -> Response {
    let path = req
        .extensions()
        .get::<OriginalUri>()
        .map(|u| u.0.path_and_query().map(|p| p.as_str().to_string()).unwrap_or_default())
        .unwrap_or_else(|| req.uri().to_string());
    let (parts, body) = req.into_parts();
    let mut headers = HeaderMap::new();
    copy_headers(&parts.headers, &mut headers);
    if let Some(host) = parts.headers.get(header::HOST) {
        headers.insert("x-forwarded-host", host.clone());
    }
    let upstream = t
        .http
        .request(parts.method, format!("{}{}", t.base, path))
        .headers(headers)
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await;
    match upstream {
        Ok(res) => {
            let mut out = Response::builder().status(res.status());
            if let Some(h) = out.headers_mut() {
                copy_headers(res.headers(), h);
            }
            out.body(Body::from_stream(res.bytes_stream().map_err(std::io::Error::other)))
                .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
        }
        Err(e) => {
            tracing::warn!(error = %e, base = %t.base, "core unreachable");
            AppError::Upstream("service temporarily unavailable".into()).into_response()
        }
    }
}
