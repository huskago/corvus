use std::path::PathBuf;

use crate::models::{LauncherConfig, LocalInstanceConfig};

pub fn base_dir() -> PathBuf {
    dirs::data_dir()
        .expect("Unable to find the OS data folder")
        .join("corvus")
}

pub fn launcher_config_path() -> PathBuf {
    base_dir().join("launcher_config.json")
}

pub fn instance_dir(game_dir_name: &str) -> PathBuf {
    base_dir().join("instances").join(game_dir_name)
}

pub fn instance_config_path(game_dir_name: &str) -> PathBuf {
    instance_dir(game_dir_name).join("user_config.json")
}

pub async fn read_launcher_config() -> Result<LauncherConfig, String> {
    let path = launcher_config_path();

    if !path.exists() {
        return Ok(LauncherConfig::default());
    }

    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Unable to read launcher_config.json: {e}"))?;

    serde_json::from_str(&content).map_err(|e| format!("Invalid JSON in launcher_config.json: {e}"))
}

pub async fn write_launcher_config(config: &LauncherConfig) -> Result<(), String> {
    let path = launcher_config_path();

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Unable to create the folders: {e}"))?;
    }

    let content =
        serde_json::to_string_pretty(config).map_err(|e| format!("Serialization error: {e}"))?;

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Unable to write to the launcher_config.json: {e}"))
}

pub async fn read_instance_config(game_dir_name: &str) -> Result<LocalInstanceConfig, String> {
    let path = instance_config_path(game_dir_name);

    if !path.exists() {
        return Ok(LocalInstanceConfig::default());
    }

    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Unable to read user_config.json: {e}"))?;

    serde_json::from_str(&content).map_err(|e| format!("Invalid JSON in user_config.json: {e}"))
}

pub async fn write_instance_config(
    game_dir_name: &str,
    config: &LocalInstanceConfig,
) -> Result<(), String> {
    let path = instance_config_path(game_dir_name);

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Unable to create the folders: {e}"))?;
    }

    let content =
        serde_json::to_string_pretty(config).map_err(|e| format!("Serialization error: {e}"))?;

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Unable to write to the user_config.json: {e}"))
}
