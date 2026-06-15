use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherBuildConfig {
    pub allow_offline: bool,
    pub launcher_name: String,
}

impl Default for LauncherBuildConfig {
    fn default() -> Self {
        Self {
            allow_offline: true,
            launcher_name: "Corvus".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub uuid: String,
    pub username: String,
    pub is_active: bool,
    pub last_used: u64,
    pub auth_mode: AuthMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Microsoft,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceInfo {
    pub name: String,
    pub game_dir_name: String,
    pub version: String,
    pub mc_version: String,
    pub loader: ModLoader,
    pub loader_version: String,
    pub icon_url: String,
    pub bg_url: Option<String>,
    pub update_url: String,
    pub server_ip: Option<String>,
    pub maintenance: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModLoader {
    Vanilla,
    Fabric,
    Forge,
    #[serde(rename = "NEOFORGE")]
    NeoForge,
    #[serde(rename = "QUILT")]
    Quilt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub news_type: NewsType,
    pub date: String,
    pub image_url: Option<String>,
    pub action_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NewsType {
    Update,
    Event,
    Maintenance,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    pub theme: String,
    pub keep_launcher_open: bool,
    pub show_console: bool,
    pub discord_rpc: bool,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            theme: "DARK".into(),
            keep_launcher_open: false,
            show_console: false,
            discord_rpc: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCodeInfo {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub expires_in: u32,
    pub interval: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResult {
    pub success: bool,
    pub account: Option<AccountInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    pub step: String,
    pub files_done: u32,
    pub files_total: u32,
    pub bytes_done: u64,
    pub speed_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalInstanceConfig {
    pub min_ram: u32,
    pub max_ram: u32,
    pub jvm_args: String,
    pub java_path: String,
    pub optimized_args: bool,
    pub resolution_width: u32,
    pub resolution_height: u32,
    pub auto_connect_server: bool,
    pub auto_backup: bool,
    pub disabled_mods: Vec<String>,
    pub enabled_mods: Vec<String>,
}

impl Default for LocalInstanceConfig {
    fn default() -> Self {
        Self {
            min_ram: 1024, max_ram: 4096,
            jvm_args: String::new(), java_path: String::new(),
            optimized_args: false,
            resolution_width: 0, resolution_height: 0,
            auto_connect_server: false, auto_backup: true,
            disabled_mods: Vec::new(), enabled_mods: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub name: String,
    #[serde(rename = "downloadURL")]
    pub download_url: String,
    pub sha1: String,
    pub size: u64,
    pub status: ModStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModStatus { Required, OptionalOn, OptionalOff }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceManifest {
    pub mods: Vec<ManifestFile>,
    pub resource_packs: Vec<ManifestFile>,
    pub shaders: Vec<ManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayHistoryStats {
    pub total_time_minutes: u64,
    pub session_count: u32,
    pub avg_session_minutes: u64,
    pub this_week_minutes: u64,
    pub last_played: Option<u64>,
}