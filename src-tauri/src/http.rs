use reqwest::{RequestBuilder, StatusCode};
use serde_json::Value;
use std::time::Duration;

/// Send a request, retrying politely on throttling (429/503).
pub async fn exec(req: RequestBuilder) -> Result<(StatusCode, Vec<u8>), String> {
    for attempt in 0..4u64 {
        let r = req.try_clone().ok_or("request cannot be retried")?;
        let res = r.send().await.map_err(|e| format!("network error: {e}"))?;
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
