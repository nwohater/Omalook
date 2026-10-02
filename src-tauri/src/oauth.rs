//! OAuth 2.0 authorization-code flow with PKCE and a loopback redirect (works for Microsoft and Google).
use crate::config::Tokens;
use crate::http;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct OAuthApp {
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: String,
    pub redirect_host: &'static str,
    pub extra: Vec<(&'static str, &'static str)>,
    /// Microsoft wants `scope` on refresh; Google rejects nothing but doesn't need it.
    pub scope_on_refresh: bool,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

fn rand_b64(n: usize) -> String {
    let mut b = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}

async fn post_token(app: &OAuthApp, mut form: Vec<(&str, String)>) -> Result<TokenResponse, String> {
    form.push(("client_id", app.client_id.clone()));
    if let Some(sec) = &app.client_secret {
        form.push(("client_secret", sec.clone()));
    }
    let v = http::json(reqwest::Client::new().post(&app.token_url).form(&form)).await?;
    serde_json::from_value(v).map_err(|e| format!("unexpected token response: {e}"))
}

fn to_tokens(r: TokenResponse, old_refresh: Option<&str>) -> Result<Tokens, String> {
    Ok(Tokens {
        access: r.access_token,
        refresh: r
            .refresh_token
            .or_else(|| old_refresh.map(String::from))
            .ok_or("no refresh token was returned (check offline access / consent settings)")?,
        expires_at: now() + r.expires_in.saturating_sub(60),
    })
}

/// Interactive sign-in. `open` is called with the URL to show in the user's browser.
pub async fn sign_in(app: &OAuthApp, open: impl FnOnce(&str)) -> Result<Tokens, String> {
    let verifier = rand_b64(48);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = rand_b64(16);

    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| e.to_string())?;
    let redirect = format!("http://{}:{}", app.redirect_host, listener.local_addr().unwrap().port());

    let mut url = format!(
        "{}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={state}&code_challenge={challenge}&code_challenge_method=S256",
        app.auth_url,
        urlencoding::encode(&app.client_id),
        urlencoding::encode(&redirect),
        urlencoding::encode(&app.scopes),
    );
    for (k, v) in &app.extra {
        url.push_str(&format!("&{k}={}", urlencoding::encode(v)));
    }
    open(&url);

    let (mut sock, _) = tokio::time::timeout(std::time::Duration::from_secs(300), listener.accept())
        .await
        .map_err(|_| "sign-in timed out")?
        .map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 8192];
    let n = sock.read(&mut buf).await.map_err(|e| e.to_string())?;
    let req = String::from_utf8_lossy(&buf[..n]).to_string();
    let target = req.split_whitespace().nth(1).unwrap_or("/");
    let query = target.split_once('?').map(|x| x.1).unwrap_or("");
    let param = |k: &str| {
        query.split('&').find_map(|kv| {
            let (a, b) = kv.split_once('=')?;
            (a == k).then(|| urlencoding::decode(&b.replace('+', " ")).unwrap_or_default().into_owned())
        })
    };

    let ok = param("code").is_some() && param("state").as_deref() == Some(&state);
    let msg = if ok { "Signed in. You can close this tab and return to Omalook." } else { "Sign-in failed. Return to Omalook for details." };
    let html = format!("<html><body style='font-family:sans-serif;background:#111;color:#eee;display:grid;place-items:center;height:100vh'><h2>{msg}</h2></body></html>");
    let _ = sock
        .write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len()).as_bytes())
        .await;

    if !ok {
        return Err(param("error_description").or_else(|| param("error")).unwrap_or_else(|| "sign-in was cancelled".into()));
    }
    let r = post_token(
        app,
        vec![
            ("grant_type", "authorization_code".into()),
            ("code", param("code").unwrap()),
            ("redirect_uri", redirect),
            ("code_verifier", verifier),
        ],
    )
    .await?;
    to_tokens(r, None)
}

pub async fn refresh(app: &OAuthApp, t: &Tokens) -> Result<Tokens, String> {
    let mut form = vec![("grant_type", "refresh_token".to_string()), ("refresh_token", t.refresh.clone())];
    if app.scope_on_refresh {
        form.push(("scope", app.scopes.clone()));
    }
    let r = post_token(app, form).await?;
    to_tokens(r, Some(&t.refresh))
}
