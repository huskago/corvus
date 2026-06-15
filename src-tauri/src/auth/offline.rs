use crate::build_config;
use crate::models::{AuthMode, AuthResult, StoredAccount};

pub fn offline_uuid(username: &str) -> String {
    let input = format!("OfflinePlayer:{username}");
    let digest = md5::compute(input.as_bytes());
    let mut b = digest.0;

    // UUID version 3 bits (RFC 4122)
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9],
        b[10], b[11], b[12], b[13], b[14], b[15],
    )
}

pub async fn add_offline_account(username: &str) -> Result<AuthResult, String> {
    if !build_config::get().auth.allow_offline {
        return Err("Offline accounts are disabled on this launcher.".to_string());
    }
    if username.trim().is_empty() {
        return Err("The username cannot be left blank".to_string());
    }
    if username.len() > 16 {
        return Err("The username cannot be longer than 16 characters".to_string());
    }

    let uuid = offline_uuid(username);
    let now = crate::history::current_time_ms();

    let account = StoredAccount {
        auth_mode: AuthMode::Offline,
        uuid: uuid.clone(),
        username: username.to_string(),
        is_active: true,
        last_used: now,
        mc_access_token: "0".to_string(),
        mc_access_token_expiry: 0,
        ms_refresh_token: String::new(),
    };

    let mut existing = super::load_stored_accounts().await;
    for acc in &mut existing {
        acc.is_active = false;
    }
    match existing.iter_mut().find(|a| a.uuid == account.uuid) {
        Some(slot) => *slot = account.clone(),
        None => existing.push(account.clone()),
    }
    super::save_stored_accounts(&existing).await?;

    let info = account.to_account_info();
    Ok(AuthResult {
        success: true,
        account: Some(info),
        error: None,
    })
}
