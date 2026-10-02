//! App credentials, saved accounts, and token storage (system keyring with a 0600 file fallback).
use crate::model::Account;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MsConfig {
    pub client_id: String,
    /// Directory (tenant) id, or `organizations` / `common` for multi-tenant registrations.
    pub tenant_id: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct AppConfig {
    pub microsoft: Option<MsConfig>,
    pub google: Option<GoogleConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Tokens {
    pub access: String,
    pub refresh: String,
    pub expires_at: u64,
}

fn dir(base_env: &str, fallback: &str) -> PathBuf {
    std::env::var_os(base_env)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(fallback))
        .join("omalook")
}

pub fn config_dir() -> PathBuf {
    dir("XDG_CONFIG_HOME", ".config")
}

pub fn data_dir() -> PathBuf {
    dir("XDG_DATA_HOME", ".local/share")
}

fn write_private(path: &PathBuf, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())
}

pub fn load_config() -> AppConfig {
    std::fs::read(config_dir().join("config.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_config(c: &AppConfig) -> Result<(), String> {
    write_private(&config_dir().join("config.json"), &serde_json::to_vec_pretty(c).unwrap())
}

pub fn load_accounts() -> Vec<Account> {
    std::fs::read(config_dir().join("accounts.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_accounts(a: &[Account]) -> Result<(), String> {
    write_private(&config_dir().join("accounts.json"), &serde_json::to_vec_pretty(a).unwrap())
}

fn token_file(id: &str) -> PathBuf {
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '@' || c == '.' { c } else { '_' }).collect();
    data_dir().join("tokens").join(format!("{safe}.json"))
}

pub fn load_tokens(id: &str) -> Option<Tokens> {
    if let Ok(entry) = keyring::Entry::new("omalook", id) {
        if let Ok(secret) = entry.get_password() {
            if let Ok(t) = serde_json::from_str(&secret) {
                return Some(t);
            }
        }
    }
    serde_json::from_slice(&std::fs::read(token_file(id)).ok()?).ok()
}

/// Returns true if the tokens went into the system keyring.
pub fn store_tokens(id: &str, t: &Tokens) -> Result<bool, String> {
    let json = serde_json::to_string(t).unwrap();
    if let Ok(entry) = keyring::Entry::new("omalook", id) {
        if entry.set_password(&json).is_ok() {
            let _ = std::fs::remove_file(token_file(id));
            return Ok(true);
        }
    }
    write_private(&token_file(id), json.as_bytes())?;
    Ok(false)
}

pub fn delete_tokens(id: &str) {
    if let Ok(entry) = keyring::Entry::new("omalook", id) {
        let _ = entry.delete_credential();
    }
    let _ = std::fs::remove_file(token_file(id));
}
