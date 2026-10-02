use reqwest::{RequestBuilder, StatusCode};
use serde_json::Value;
use std::time::Duration;

/// Send a request, retrying politely on throttling (429/503).
pub async fn exec(req: RequestBuilder) -> Result<(StatusCode, Vec<u8>), String> {
    let (client, built) = req.build_split();
    let mut built = built.map_err(|e| e.to_string())?;
    // POST/PUT/PATCH without a body go out with no Content-Length, which Google and
    // Microsoft reject with "411 Length Required"
    if built.body().is_none() && matches!(built.method().as_str(), "POST" | "PUT" | "PATCH") {
        built.headers_mut().insert(reqwest::header::CONTENT_LENGTH, 0.into());
    }
    for attempt in 0..4u64 {
        let r = built.try_clone().ok_or("request cannot be retried")?;
        let res = client.execute(r).await.map_err(|e| format!("network error: {e}"))?;
        let status = res.status();
        if (status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE) && attempt < 3 {
            let wait = res
                .headers()
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(attempt + 1)
                .min(30);
            tokio::time::sleep(Duration::from_secs(wait)).await;
            continue;
        }
        let bytes = res.bytes().await.map_err(|e| e.to_string())?.to_vec();
        return Ok((status, bytes));
    }
    Err("service is busy, try again shortly".into())
}

fn error_message(status: StatusCode, bytes: &[u8]) -> String {
    let v: Value = serde_json::from_slice(bytes).unwrap_or_default();
    v["error"]["message"]
        .as_str()
        .or_else(|| v["error_description"].as_str())
        .map(String::from)
        .unwrap_or_else(|| format!("request failed ({status})"))
}

/// Send a request and parse a JSON response (empty bodies become `null`).
pub async fn json(req: RequestBuilder) -> Result<Value, String> {
    let (status, bytes) = exec(req).await?;
    if !status.is_success() {
        return Err(error_message(status, &bytes));
    }
    Ok(serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

pub async fn bytes(req: RequestBuilder) -> Result<Vec<u8>, String> {
    let (status, bytes) = exec(req).await?;
    if !status.is_success() {
        return Err(error_message(status, &bytes));
    }
    Ok(bytes)
}

/// Everything a provider call needs.
pub struct Ctx<'a> {
    pub http: &'a reqwest::Client,
    pub token: &'a str,
    pub account: &'a crate::model::Account,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serve one request and return the raw bytes the client sent.
    async fn capture(method: reqwest::Method, with_body: bool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let n = s.read(&mut buf).await.unwrap();
            s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").await.unwrap();
            String::from_utf8_lossy(&buf[..n]).to_lowercase()
        });
        let mut req = reqwest::Client::new().request(method, format!("http://127.0.0.1:{port}/x"));
        if with_body {
            req = req.json(&serde_json::json!({ "a": 1 }));
        }
        json(req).await.unwrap();
        server.await.unwrap()
    }

    #[tokio::test]
    async fn bodiless_post_declares_zero_length() {
        // Google and Microsoft front-ends answer "411 Length Required" otherwise
        let raw = capture(reqwest::Method::POST, false).await;
        assert!(raw.contains("content-length: 0"), "request was:\n{raw}");
    }

    #[tokio::test]
    async fn bodiless_get_is_left_alone() {
        let raw = capture(reqwest::Method::GET, false).await;
        assert!(!raw.contains("content-length"), "request was:\n{raw}");
    }

    #[tokio::test]
    async fn json_post_keeps_its_real_length() {
        let raw = capture(reqwest::Method::POST, true).await;
        assert!(raw.contains("content-length: 7"), "request was:\n{raw}");
    }
}
