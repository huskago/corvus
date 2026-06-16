use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;

use super::upsert_account;
use crate::build_config;
use crate::models::{AuthMode, AuthResult, DeviceCodeInfo, StoredAccount};
use crate::state::AppStateInner;

fn client_id() -> &'static str {
    &build_config::get().auth.microsoft_client_id
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    user_code: String,
    device_code: String,
    verification_uri: String,
    expires_in: u32,
    interval: u32,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct XblRequest {
    #[serde(rename = "Properties")]
    properties: XblProperties,
    #[serde(rename = "RelyingParty")]
    relying_party: String,
    #[serde(rename = "TokenType")]
    token_type: String,
}

#[derive(Serialize)]
struct XblProperties {
    #[serde(rename = "AuthMethod")]
    auth_method: String,
    #[serde(rename = "SiteName")]
    site_name: String,
    #[serde(rename = "RpsTicket")]
    rps_ticket: String,
}

#[derive(Deserialize)]
struct XblResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XblClaims,
}

#[derive(Deserialize)]
struct XblClaims {
    xui: Vec<XuiEntry>,
}

#[derive(Deserialize)]
struct XuiEntry {
    uhs: String,
}

#[derive(Serialize)]
struct XstsRequest {
    #[serde(rename = "Properties")]
    properties: XstsProperties,
    #[serde(rename = "RelyingParty")]
    relying_party: String,
    #[serde(rename = "TokenType")]
    token_type: String,
}

#[derive(Serialize)]
struct XstsProperties {
    #[serde(rename = "SandboxId")]
    sandbox_id: String,
    #[serde(rename = "UserTokens")]
    user_tokens: Vec<String>,
}

#[derive(Deserialize)]
struct XstsResponse {
    #[serde(rename = "Token")]
    token: String,
}

#[derive(Serialize)]
struct McLoginRequest {
    #[serde(rename = "identityToken")]
    identity_token: String,
}

#[derive(Deserialize)]
struct McLoginResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct McProfileResponse {
    id: String,
    name: String,
}

pub async fn start_device_code_flow(client: &reqwest::Client) -> Result<DeviceCodeInfo, String> {
    let resp: DeviceCodeResponse = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[
            ("client_id", client_id()),
            ("scope", "XboxLive.signin XboxLive.offline_access"),
        ])
        .send()
        .await
        .map_err(|e| format!("DeviceCode request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("DeviceCode response invalid: {e}"))?;

    Ok(DeviceCodeInfo {
        user_code: resp.user_code,
        device_code: resp.device_code,
        verification_uri: resp.verification_uri,
        expires_in: resp.expires_in,
        interval: resp.interval,
    })
}

pub async fn poll_until_done(
    client: &reqwest::Client,
    device_code: &str,
    interval: u32,
    state: &AppStateInner,
) -> Result<AuthResult, String> {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval as u64)).await;

        if state.auth_poll_cancelled.load(Ordering::Relaxed) {
            return Ok(AuthResult {
                success: false,
                account: None,
                error: Some("Authentication cancelled".to_string()),
            });
        }

        let token_resp: TokenResponse = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id()),
                ("device_code", device_code),
            ])
            .send()
            .await
            .map_err(|e| format!("Polling request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Polling response invalid: {e}"))?;

        match token_resp.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
            Some(err) => return Err(format!("Microsoft authentication failed: {err}")),
            None => {
                let ms_token = token_resp
                    .access_token
                    .ok_or("Microsoft token missing from the response")?;
                let ms_refresh = token_resp
                    .refresh_token
                    .ok_or("Refresh token missing from the response")?;

                let account = complete_auth_chain(client, &ms_token, &ms_refresh).await?;

                let info = account.to_account_info();
                upsert_account(account).await?;

                return Ok(AuthResult {
                    success: true,
                    account: Some(info),
                    error: None,
                });
            }
        }
    }
}

async fn complete_auth_chain(
    client: &reqwest::Client,
    ms_token: &str,
    ms_refresh: &str,
) -> Result<StoredAccount, String> {
    // Xbox Live
    let xbl: XblResponse = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&XblRequest {
            properties: XblProperties {
                auth_method: "RPS".to_string(),
                site_name: "user.auth.xboxlive.com".to_string(),
                rps_ticket: format!("d={ms_token}"),
            },
            relying_party: "http://auth.xboxlive.com".to_string(),
            token_type: "JWT".to_string(),
        })
        .send()
        .await
        .map_err(|e| format!("XBL auth failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("XBL response invalid: {e}"))?;

    let uhs = xbl
        .display_claims
        .xui
        .into_iter()
        .next()
        .ok_or("XBL: missing uhs")?
        .uhs;

    // XSTS
    let xsts: XstsResponse = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&XstsRequest {
            properties: XstsProperties {
                sandbox_id: "RETAIL".to_string(),
                user_tokens: vec![xbl.token],
            },
            relying_party: "rp://api.minecraftservices.com/".to_string(),
            token_type: "JWT".to_string(),
        })
        .send()
        .await
        .map_err(|e| format!("XSTS auth failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("XSTS response invalid: {e}"))?;

    // Minecraft
    let mc_login: McLoginResponse = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&McLoginRequest {
            identity_token: format!("XBL3.0 x={uhs};{}", xsts.token),
        })
        .send()
        .await
        .map_err(|e| format!("MC auth failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("MC auth response invalid: {e}"))?;

    // Minecraft profile
    let profile: McProfileResponse = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(&mc_login.access_token)
        .send()
        .await
        .map_err(|e| format!("Profile request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Profile response invalid: {e}"))?;

    let now_ms = crate::history::current_time_ms();
    let expiry_ms = now_ms + mc_login.expires_in * 1000;

    let raw = &profile.id;
    if raw.len() < 32 {
        return Err(format!("Unexpected profile ID: {raw}"));
    }
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &raw[0..8],
        &raw[8..12],
        &raw[12..16],
        &raw[16..20],
        &raw[20..]
    );

    Ok(StoredAccount {
        auth_mode: AuthMode::Microsoft,
        uuid,
        username: profile.name,
        is_active: true,
        last_used: now_ms,
        mc_access_token: mc_login.access_token,
        mc_access_token_expiry: expiry_ms,
        ms_refresh_token: ms_refresh.to_string(),
    })
}

pub async fn refresh_token(client: &reqwest::Client, uuid: &str) -> Result<bool, String> {
    let mut accounts = super::load_stored_accounts().await;
    let account = accounts
        .iter_mut()
        .find(|a| a.uuid == uuid)
        .ok_or_else(|| format!("Account '{}' not found", uuid))?;

    if account.auth_mode != AuthMode::Microsoft {
        return Ok(false);
    }

    let resp: TokenResponse = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id()),
            ("refresh_token", account.ms_refresh_token.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Refresh request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Refresh response invalid: {e}"))?;

    if resp.error.is_some() {
        return Ok(false);
    }

    let ms_token = resp.access_token.ok_or("Token missing")?;
    let ms_refresh = resp.refresh_token.ok_or("Refresh token missing")?;

    let new_account = complete_auth_chain(client, &ms_token, &ms_refresh).await?;

    account.mc_access_token = new_account.mc_access_token;
    account.mc_access_token_expiry = new_account.mc_access_token_expiry;
    account.ms_refresh_token = new_account.ms_refresh_token;
    account.last_used = new_account.last_used;

    super::save_stored_accounts(&accounts).await?;
    Ok(true)
}

impl PartialEq for AuthMode {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (AuthMode::Microsoft, AuthMode::Microsoft) | (AuthMode::Offline, AuthMode::Offline)
        )
    }
}
