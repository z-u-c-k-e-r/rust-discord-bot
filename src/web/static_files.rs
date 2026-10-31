use std::sync::LazyLock;

use axum::{
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
    },
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};

const INDEX_HTML: &str = include_str!("../../web/static/index.html");
const STYLES_CSS: &str = include_str!("../../web/static/styles.css");
const APP_JS: &str = include_str!("../../web/static/app.js");
const FAVICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <defs>
    <linearGradient id="g" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#5865F2"/>
      <stop offset="100%" stop-color="#4752C4"/>
    </linearGradient>
  </defs>
  <rect width="100" height="100" rx="24" fill="url(#g)"/>
  <path d="M30 40 C30 32, 42 32, 50 32 C58 32, 70 32, 70 40 C70 56, 62 68, 50 68 C38 68, 30 56, 30 40 Z" fill="#ffffff" fill-opacity="0.95"/>
  <circle cx="42" cy="46" r="4.5" fill="#5865F2"/>
  <circle cx="58" cy="46" r="4.5" fill="#5865F2"/>
  <path d="M44 56 Q50 60 56 56" stroke="#5865F2" stroke-width="2.5" stroke-linecap="round" fill="none"/>
  <circle cx="74" cy="26" r="6" fill="#57F287"/>
</svg>"##;

static INDEX_ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(INDEX_HTML.as_bytes()));
static STYLES_ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(STYLES_CSS.as_bytes()));
static APP_JS_ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(APP_JS.as_bytes()));
static FAVICON_ETAG: LazyLock<String> = LazyLock::new(|| compute_etag(FAVICON_SVG.as_bytes()));

fn compute_etag(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let hash = hex::encode(hasher.finalize());
    format!("\"{}\"", &hash[..16])
}

pub async fn index(headers: HeaderMap) -> Response {
    serve_static(
        &headers,
        INDEX_HTML,
        "text/html; charset=utf-8",
        &INDEX_ETAG,
        "no-cache, must-revalidate",
    )
}

pub async fn styles(headers: HeaderMap) -> Response {
    serve_static(
        &headers,
        STYLES_CSS,
        "text/css; charset=utf-8",
        &STYLES_ETAG,
        "public, max-age=300, stale-while-revalidate=60",
    )
}

pub async fn javascript(headers: HeaderMap) -> Response {
    serve_static(
        &headers,
        APP_JS,
        "text/javascript; charset=utf-8",
        &APP_JS_ETAG,
        "public, max-age=300, stale-while-revalidate=60",
    )
}

pub async fn favicon(headers: HeaderMap) -> Response {
    serve_static(
        &headers,
        FAVICON_SVG,
        "image/svg+xml",
        &FAVICON_ETAG,
        "public, max-age=86400",
    )
}

fn serve_static(
    headers: &HeaderMap,
    content: &'static str,
    content_type: &'static str,
    etag: &str,
    cache_control: &'static str,
) -> Response {
    if let Some(if_none_match) = headers.get(IF_NONE_MATCH).and_then(|h| h.to_str().ok())
        && (if_none_match == etag || if_none_match == "*")
    {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        if let Ok(etag_val) = HeaderValue::from_str(etag) {
            response.headers_mut().insert(ETAG, etag_val);
        }
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static(cache_control));
        return response;
    }

    let mut response = content.into_response();
    let h = response.headers_mut();
    h.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    h.insert(CACHE_CONTROL, HeaderValue::from_static(cache_control));
    if let Ok(etag_val) = HeaderValue::from_str(etag) {
        h.insert(ETAG, etag_val);
    }
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}
