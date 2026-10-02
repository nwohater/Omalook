mod config;
mod google;
mod http;
mod model;
mod ms;
mod oauth;
mod theme;
mod util;

use config::{AppConfig, Tokens};
use http::Ctx;
use model::*;
use std::collections::HashMap;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;

const MS_SCOPES: &str = "https://graph.microsoft.com/Mail.ReadWrite https://graph.microsoft.com/Mail.Send https://graph.microsoft.com/Calendars.ReadWrite https://graph.microsoft.com/Contacts.ReadWrite https://graph.microsoft.com/User.Read offline_access";
const GOOGLE_SCOPES: &str = "openid email profile https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/calendar https://www.googleapis.com/auth/contacts";

pub struct AppState {
    tokens: Mutex<HashMap<String, Tokens>>,
    accounts: Mutex<Vec<Account>>,
    http: reqwest::Client,
}

fn oauth_app(kind: Kind) -> Result<oauth::OAuthApp, String> {
    let cfg = config::load_config();
    match kind {
        Kind::Microsoft => {
            let c = cfg.microsoft.filter(|c| !c.client_id.is_empty()).ok_or("Microsoft 365 isn't set up yet. Open Settings → Accounts and follow the guide.")?;
            let tenant = if c.tenant_id.is_empty() { "organizations".to_string() } else { c.tenant_id };
            Ok(oauth::OAuthApp {
                auth_url: format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize"),
                token_url: format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token"),
                client_id: c.client_id,
                client_secret: None,
                scopes: MS_SCOPES.into(),
                redirect_host: "localhost",
                extra: vec![("response_mode", "query"), ("prompt", "select_account")],
                scope_on_refresh: true,
            })
        }
        Kind::Google => {
            let c = cfg.google.filter(|c| !c.client_id.is_empty()).ok_or("Google isn't set up yet. Open Settings → Accounts and follow the guide.")?;
            Ok(oauth::OAuthApp {
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
                token_url: "https://oauth2.googleapis.com/token".into(),
                client_id: c.client_id,
                client_secret: Some(c.client_secret).filter(|s| !s.is_empty()),
                scopes: GOOGLE_SCOPES.into(),
                redirect_host: "127.0.0.1",
                extra: vec![("access_type", "offline"), ("prompt", "consent select_account")],
                scope_on_refresh: false,
            })
        }
    }
}

/// Resolve an account and a valid access token, refreshing if needed.
async fn session(state: &AppState, id: &str) -> Result<(Account, String), String> {
    let account = state.accounts.lock().await.iter().find(|a| a.id == id).cloned().ok_or("unknown account")?;
    let mut toks = state.tokens.lock().await;
    let t = match toks.get(id).cloned() {
        Some(t) => t,
        None => {
            let t = config::load_tokens(id).ok_or("Signed out. Add the account again in Settings.")?;
            toks.insert(id.to_string(), t.clone());
            t
        }
    };
    if t.expires_at > oauth::now() {
        return Ok((account, t.access));
    }
    let app = oauth_app(account.kind)?;
    let nt = oauth::refresh(&app, &t).await.map_err(|e| format!("Session expired — remove and re-add this account ({e})"))?;
    config::store_tokens(id, &nt)?;
    toks.insert(id.to_string(), nt.clone());
    Ok((account, nt.access))
}

macro_rules! with_ctx {
    ($state:expr, $id:expr, $c:ident, $body:expr) => {{
        let (account, token) = session(&$state, &$id).await?;
        let $c = Ctx { http: &$state.http, token: &token, account: &account };
        $body
    }};
}

macro_rules! by_kind {
    ($c:ident, $f:ident ( $($arg:expr),* )) => {
        match $c.account.kind {
            Kind::Microsoft => ms::$f(&$c $(, $arg)*).await,
            Kind::Google => google::$f(&$c $(, $arg)*).await,
        }
    };
}

// ───────────────────────── config & accounts ─────────────────────────

#[tauri::command]
fn get_config() -> AppConfig {
    config::load_config()
}

#[tauri::command]
fn save_config(config: AppConfig) -> Result<(), String> {
    config::save_config(&config)
}

#[tauri::command]
async fn list_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, ()> {
    Ok(state.accounts.lock().await.clone())
}

#[tauri::command]
async fn add_account(app: AppHandle, state: State<'_, AppState>, kind: Kind) -> Result<Account, String> {
    let oapp = oauth_app(kind)?;
    let handle = app.clone();
    let tokens = oauth::sign_in(&oapp, move |url| {
        let _ = handle.opener().open_url(url, None::<&str>);
    })
    .await?;

    let blank = Account { id: String::new(), kind, email: String::new(), name: String::new() };
    let c = Ctx { http: &state.http, token: &tokens.access, account: &blank };
    let (email, name) = match kind {
        Kind::Microsoft => ms::profile(&c).await.map(|p| (p.email, p.name))?,
        Kind::Google => google::profile(&c).await.map(|p| (p.email, p.name))?,
    };
    let account = Account { id: format!("{}:{}", kind.as_str(), email.to_lowercase()), kind, email, name };
    config::store_tokens(&account.id, &tokens)?;
    state.tokens.lock().await.insert(account.id.clone(), tokens);
    let mut accounts = state.accounts.lock().await;
    accounts.retain(|a| a.id != account.id);
    accounts.push(account.clone());
    config::save_accounts(&accounts)?;
    Ok(account)
}

#[tauri::command]
async fn remove_account(state: State<'_, AppState>, id: String) -> Result<(), String> {
    config::delete_tokens(&id);
    state.tokens.lock().await.remove(&id);
    let mut accounts = state.accounts.lock().await;
    accounts.retain(|a| a.id != id);
    config::save_accounts(&accounts)
}

// ───────────────────────────── mail ─────────────────────────────

#[tauri::command]
async fn list_folders(state: State<'_, AppState>, account: String) -> Result<Vec<Folder>, String> {
    with_ctx!(state, account, c, by_kind!(c, folders()))
}

#[tauri::command]
async fn list_messages(state: State<'_, AppState>, account: String, folder: String, page: Option<String>, search: Option<String>) -> Result<MessagePage, String> {
    with_ctx!(state, account, c, by_kind!(c, messages(&folder, page, search)))
}

#[tauri::command]
async fn get_message(state: State<'_, AppState>, account: String, id: String) -> Result<MsgDetail, String> {
    with_ctx!(state, account, c, by_kind!(c, message(&id)))
}

#[tauri::command]
async fn set_read(state: State<'_, AppState>, account: String, id: String, read: bool) -> Result<(), String> {
    with_ctx!(state, account, c, by_kind!(c, set_read(&id, read)))
}

#[tauri::command]
async fn set_flag(state: State<'_, AppState>, account: String, id: String, flagged: bool) -> Result<(), String> {
    with_ctx!(state, account, c, by_kind!(c, set_flag(&id, flagged)))
}

#[tauri::command]
async fn move_message(state: State<'_, AppState>, account: String, id: String, target: String, from: Option<String>) -> Result<(), String> {
    with_ctx!(state, account, c, match c.account.kind {
        Kind::Microsoft => ms::move_to(&c, &id, &target).await,
        Kind::Google => google::move_to(&c, &id, &target, from.as_deref()).await,
    })
}

#[tauri::command]
async fn delete_message(state: State<'_, AppState>, account: String, id: String) -> Result<(), String> {
    with_ctx!(state, account, c, by_kind!(c, delete_forever(&id)))
}

#[tauri::command]
async fn save_draft(state: State<'_, AppState>, account: String, out: Outgoing) -> Result<String, String> {
    with_ctx!(state, account, c, by_kind!(c, save_draft(&out)))
}

#[tauri::command]
async fn send_mail(state: State<'_, AppState>, account: String, out: Outgoing) -> Result<(), String> {
    with_ctx!(state, account, c, by_kind!(c, send(&out)))
}

#[tauri::command]
async fn open_draft(state: State<'_, AppState>, account: String, id: String) -> Result<DraftData, String> {
    with_ctx!(state, account, c, by_kind!(c, open_draft(&id)))
}

#[tauri::command]
async fn discard_draft(state: State<'_, AppState>, account: String, draft_id: String) -> Result<(), String> {
    with_ctx!(state, account, c, match c.account.kind {
        Kind::Microsoft => ms::delete_forever(&c, &draft_id).await,
        Kind::Google => google::discard_draft(&c, &draft_id).await,
    })
}

fn downloads_dir() -> std::path::PathBuf {
    dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()))
}

fn unique_path(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let clean: String = name.chars().map(|c| if matches!(c, '/' | '\\' | '\0') { '_' } else { c }).collect();
    let clean = clean.trim_start_matches('.');
    let clean = if clean.is_empty() { "attachment" } else { clean };
    let (stem, ext) = match clean.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (clean.to_string(), String::new()),
    };
    let mut p = dir.join(clean);
    let mut n = 1;
    while p.exists() {
        p = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    p
}

#[tauri::command]
async fn download_attachment(state: State<'_, AppState>, account: String, message: String, attachment: String, name: String) -> Result<String, String> {
    let bytes = with_ctx!(state, account, c, by_kind!(c, attachment_bytes(&message, &attachment)))?;
    let dir = downloads_dir();
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let path = unique_path(&dir, &name);
    tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Open a file we saved into the Downloads folder (refuses anything outside it).
#[tauri::command]
fn open_download(app: AppHandle, path: String) -> Result<(), String> {
    let p = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
    let dir = std::fs::canonicalize(downloads_dir()).map_err(|e| e.to_string())?;
    if !p.starts_with(&dir) {
        return Err("refusing to open a file outside Downloads".into());
    }
    app.opener().open_path(p.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

// ─────────────────────── calendar & contacts ───────────────────────

#[tauri::command]
async fn list_calendars(state: State<'_, AppState>, account: String) -> Result<Vec<CalendarInfo>, String> {
    with_ctx!(state, account, c, by_kind!(c, calendars()))
}

#[tauri::command]
async fn list_events(state: State<'_, AppState>, account: String, from: String, to: String) -> Result<Vec<Event>, String> {
    with_ctx!(state, account, c, by_kind!(c, events(&from, &to)))
}

#[tauri::command]
async fn save_event(state: State<'_, AppState>, account: String, event: EventInput) -> Result<(), String> {
    with_ctx!(state, account, c, by_kind!(c, save_event(&event)))
}

#[tauri::command]
async fn delete_event(state: State<'_, AppState>, account: String, calendar_id: String, id: String) -> Result<(), String> {
    with_ctx!(state, account, c, match c.account.kind {
        Kind::Microsoft => ms::delete_event(&c, &id).await,
        Kind::Google => google::delete_event(&c, &calendar_id, &id).await,
    })
}

#[tauri::command]
async fn list_contacts(state: State<'_, AppState>, account: String) -> Result<Vec<Contact>, String> {
    with_ctx!(state, account, c, by_kind!(c, contacts()))
}

#[tauri::command]
async fn save_contact(state: State<'_, AppState>, account: String, contact: Contact) -> Result<(), String> {
    if contact.id.contains("..") || (!contact.id.is_empty() && account.starts_with("google:") && !contact.id.starts_with("people/")) {
        return Err("invalid contact id".into());
    }
    with_ctx!(state, account, c, by_kind!(c, save_contact(&contact)))
}

#[tauri::command]
async fn delete_contact(state: State<'_, AppState>, account: String, id: String) -> Result<(), String> {
    if id.contains("..") || (account.starts_with("google:") && !id.starts_with("people/")) {
        return Err("invalid contact id".into());
    }
    with_ctx!(state, account, c, by_kind!(c, delete_contact(&id)))
}

#[tauri::command]
fn theme_colors() -> Option<HashMap<String, String>> {
    theme::colors()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            tokens: Mutex::new(HashMap::new()),
            accounts: Mutex::new(config::load_accounts()),
            http: reqwest::Client::builder().user_agent("Omalook").build().expect("http client"),
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            list_accounts,
            add_account,
            remove_account,
            list_folders,
            list_messages,
            get_message,
            set_read,
            set_flag,
            move_message,
            delete_message,
            save_draft,
            send_mail,
            open_draft,
            discard_draft,
            download_attachment,
            open_download,
            list_calendars,
            list_events,
            save_event,
            delete_event,
            list_contacts,
            save_contact,
            delete_contact,
            theme_colors,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
