use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BuildConfig {
    pub branding: Branding,
    pub server: Server,
    pub auth: Auth,
}

#[derive(Debug, Deserialize)]
pub struct Branding {
    pub name: String,
    pub discord_client_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Server {
    pub instances_url: String,
    pub news_url: String,
    pub updates_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Auth {
    pub microsoft_client_id: String,
    pub allow_offline: bool,
}

static CONFIG: OnceLock<BuildConfig> = OnceLock::new();

pub fn get() -> &'static BuildConfig {
    CONFIG.get_or_init(|| {
        toml::from_str(include_str!("../launcher.toml"))
            .expect("launcher.toml is missing or invalid, check src-tauri/launcher.toml")
    })
}
