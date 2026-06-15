pub mod microsoft;
pub mod offline;

use crate::config::base_dir;
use crate::models::{AccountInfo, AuthMode, StoredAccount};

const ACCOUNTS_FILE: &str = "accounts.json";

fn accounts_path() -> std::path::PathBuf {
    base_dir().join(ACCOUNTS_FILE)
}

pub async fn load_stored_accounts() -> Vec<StoredAccount> {
    let path = accounts_path();
    if !path.exists() {
        return Vec::new();
    }
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub async fn save_stored_accounts(accounts: &[StoredAccount]) -> Result<(), String> {
    let path = accounts_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let content = serde_json::to_string_pretty(accounts)
        .map_err(|e| format!("Account serialisation: {e}"))?;
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Write to disk: {e}"))
}

pub async fn upsert_account(account: StoredAccount) -> Result<(), String> {
    let mut accounts = load_stored_accounts().await;
    if let Some(existing) = accounts.iter_mut().find(|a| a.uuid == account.uuid) {
        *existing = account;
    } else {
        accounts.push(account);
    }
    save_stored_accounts(&accounts).await
}

pub async fn get_accounts() -> Vec<AccountInfo> {
    load_stored_accounts()
        .await
        .iter()
        .map(|a| a.to_account_info())
        .collect()
}

pub async fn get_active_account() -> Option<AccountInfo> {
    load_stored_accounts()
        .await
        .iter()
        .find(|a| a.is_active)
        .map(|a| a.to_account_info())
}

pub async fn set_active_account(uuid: &str) -> Result<(), String> {
    let mut accounts = load_stored_accounts().await;
    for acc in &mut accounts {
        acc.is_active = acc.uuid == uuid;
    }
    if !accounts.iter().any(|a| a.uuid == uuid) {
        return Err(format!("Account '{}' not found", uuid));
    }
    save_stored_accounts(&accounts).await
}

pub async fn remove_account(uuid: &str) -> Result<(), String> {
    let mut accounts = load_stored_accounts().await;
    let before = accounts.len();
    accounts.retain(|a| a.uuid != uuid);
    if accounts.len() == before {
        return Err(format!("Account '{}' not found", uuid));
    }
    if !accounts.is_empty() && !accounts.iter().any(|a| a.is_active) {
        accounts[0].is_active = true;
    }
    save_stored_accounts(&accounts).await
}

pub async fn get_launch_credentials() -> Option<(AuthMode, String, String, String)> {
    let accounts = load_stored_accounts().await;
    let active = accounts.into_iter().find(|a| a.is_active)?;
    Some((
        active.auth_mode,
        active.username,
        active.uuid,
        active.mc_access_token,
    ))
}
