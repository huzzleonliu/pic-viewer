use super::sandbox::{mime_for, resolve_path, to_rel};

#[cfg(feature = "ssr")]
fn parse_bytes_range(header: Option<&axum::http::HeaderValue>, len: u64) -> Option<(u64, u64)> {
    let raw = header?.to_str().ok()?.trim();
    let rest = raw.strip_prefix("bytes=")?;
    let first = rest.split(',').next()?.trim();
    let (start_s, end_s) = first.split_once('-')?;
    if start_s.is_empty() {
        let n: u64 = end_s.parse().ok()?;
        if n == 0 || len == 0 {
            return None;
        }
        let start = len.saturating_sub(n);
        return Some((start, len - 1));
    }
    let start: u64 = start_s.parse().ok()?;
    if start >= len {
        return None;
    }
    let end = if end_s.is_empty() {
        len - 1
    } else {
        end_s.parse::<u64>().ok()?.min(len - 1)
    };
    if end < start {
        return None;
    }
    Some((start, end))
}

#[cfg(feature = "ssr")]
async fn stream_file(
    path: &std::path::Path,
    mime: &'static str,
    range_header: Option<&axum::http::HeaderValue>,
) -> axum::response::Response {
    use axum::body::Body;
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::IntoResponse;
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    use tokio_util::io::ReaderStream;

    let meta = match tokio::fs::metadata(path).await {
        Ok(meta) if meta.is_file() => meta,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let len = meta.len();
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let mut res = if let Some((start, end)) = parse_bytes_range(range_header, len) {
        if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
            return StatusCode::NOT_FOUND.into_response();
        }
        let take = end - start + 1;
        let mut res = Body::from_stream(ReaderStream::new(file.take(take))).into_response();
        *res.status_mut() = StatusCode::PARTIAL_CONTENT;
        res.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{len}"))
                .unwrap_or_else(|_| HeaderValue::from_static("bytes */0")),
        );
        res.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&take.to_string()).unwrap_or(HeaderValue::from_static("0")),
        );
        res
    } else {
        let mut res = Body::from_stream(ReaderStream::new(file)).into_response();
        res.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&len.to_string()).unwrap_or(HeaderValue::from_static("0")),
        );
        res
    };
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    res.headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    res.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, max-age=120"),
    );
    res
}

#[cfg(feature = "ssr")]
pub async fn serve_media(
    axum::extract::Path(path): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    use axum::http::header;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let Ok(safe) = resolve_path(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !safe.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    stream_file(&safe, mime_for(&safe), headers.get(header::RANGE)).await
}

#[cfg(feature = "ssr")]
fn imgproxy_base() -> Option<String> {
    let raw = std::env::var("IMGPROXY_URL").ok()?;
    let base = raw.trim().trim_end_matches('/');
    if base.is_empty() {
        None
    } else {
        Some(base.to_string())
    }
}

#[cfg(feature = "ssr")]
fn imgproxy_fetch_url(base: &str, rel: &str, processing: &str) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let source = format!("local:///{}", rel.trim_start_matches('/'));
    let encoded = URL_SAFE_NO_PAD.encode(source.as_bytes());
    format!("{base}/insecure/{processing}/{encoded}")
}

#[cfg(feature = "ssr")]
const THUMB_PROCESSING: &str = "rs:fill:184:128:0/g:ce/q:70/f:webp";
#[cfg(feature = "ssr")]
const PREVIEW_PROCESSING: &str = "rs:fit:1920:1920:0/q:80/f:webp";

#[cfg(feature = "ssr")]
fn http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("http client")
    })
}

#[cfg(feature = "ssr")]
fn imgproxy_slots() -> &'static tokio::sync::Semaphore {
    use std::sync::OnceLock;
    static SLOTS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    SLOTS.get_or_init(|| tokio::sync::Semaphore::new(16))
}

#[cfg(feature = "ssr")]
async fn serve_imgproxy(path: String, processing: &'static str) -> axum::response::Response {
    use axum::body::Body;
    use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
    use axum::response::IntoResponse;

    let Some(base) = imgproxy_base() else {
        return serve_media(axum::extract::Path(path), HeaderMap::new()).await;
    };

    let Ok(safe) = resolve_path(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !safe.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let rel = to_rel(&safe);
    if rel.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let _permit = match imgproxy_slots().acquire().await {
        Ok(permit) => permit,
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    };

    let url = imgproxy_fetch_url(&base, &rel, processing);
    match http_client().get(&url).send().await {
        Ok(upstream) => {
            let status =
                StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let content_type = upstream
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| HeaderValue::from_bytes(v.as_bytes()).ok());
            let cache_control = upstream
                .headers()
                .get(reqwest::header::CACHE_CONTROL)
                .and_then(|v| HeaderValue::from_bytes(v.as_bytes()).ok());
            let etag = upstream
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|v| HeaderValue::from_bytes(v.as_bytes()).ok());
            let body = Body::from_stream(upstream.bytes_stream());
            let mut res = body.into_response();
            *res.status_mut() = status;
            let headers = res.headers_mut();
            if let Some(v) = content_type {
                headers.insert(header::CONTENT_TYPE, v);
            }
            if let Some(v) = cache_control {
                headers.insert(header::CACHE_CONTROL, v);
            } else {
                headers.insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("private, max-age=120"),
                );
            }
            if let Some(v) = etag {
                headers.insert(header::ETAG, v);
            }
            res
        }
        Err(_) => StatusCode::BAD_GATEWAY.into_response(),
    }
}

#[cfg(feature = "ssr")]
pub async fn serve_thumb(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> axum::response::Response {
    serve_imgproxy(path, THUMB_PROCESSING).await
}

#[cfg(feature = "ssr")]
pub async fn serve_preview(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> axum::response::Response {
    serve_imgproxy(path, PREVIEW_PROCESSING).await
}
