use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

mod auth;
pub mod build_config;
pub mod config;
pub mod game;
mod history;
pub mod instances;
pub mod java;
pub mod models;
pub mod news;
pub mod state;

#[derive(serde::Serialize)]
struct FrontendBuildConfig {
    allow_offline: bool,
    launcher_name: String,
}

#[tauri::command]
fn get_launcher_build_config() -> FrontendBuildConfig {
    let cfg = build_config::get();
    FrontendBuildConfig {
        allow_offline: cfg.auth.allow_offline,
        launcher_name: cfg.branding.name.clone(),
    }
}

#[tauri::command]
async fn get_launcher_config() -> Result<models::LauncherConfig, String> {
    config::read_launcher_config().await
}

#[tauri::command]
async fn save_launcher_config(config: models::LauncherConfig) -> Result<(), String> {
    config::write_launcher_config(&config).await
}

#[tauri::command]
async fn get_instance_local_config(
    game_dir_name: String,
) -> Result<models::LocalInstanceConfig, String> {
    config::read_instance_config(&game_dir_name).await
}

#[tauri::command]
async fn save_instance_local_config(
    game_dir_name: String,
    config: models::LocalInstanceConfig,
) -> Result<(), String> {
    config::write_instance_config(&game_dir_name, &config).await
}

#[tauri::command]
async fn fetch_news(
    app_state: tauri::State<'_, state::AppState>,
) -> Result<Vec<models::NewsItem>, String> {
    news::get_news(&app_state).await
}

#[tauri::command]
async fn invalidate_news_cache(app_state: tauri::State<'_, state::AppState>) -> Result<(), String> {
    news::invalidate(&app_state).await;
    Ok(())
}

#[tauri::command]
async fn fetch_instances(
    app_state: tauri::State<'_, state::AppState>,
) -> Result<Vec<models::InstanceInfo>, String> {
    instances::get_instances(&app_state).await
}

#[tauri::command]
async fn fetch_instance_manifest(
    update_url: String,
    app_state: tauri::State<'_, state::AppState>,
) -> Result<models::InstanceManifest, String> {
    instances::fetch_manifest(&app_state.http_client, &update_url).await
}

#[tauri::command]
async fn detect_java(
    java_path_override: Option<String>,
    mc_version: String,
) -> Result<Option<models::JavaInfo>, String> {
    Ok(java::detect_java(java_path_override.as_deref(), &mc_version).await)
}

#[tauri::command]
async fn download_java(
    mc_version: String,
    app_state: tauri::State<'_, state::AppState>,
) -> Result<String, String> {
    java::download_java(&app_state.http_client, &mc_version).await
}

#[tauri::command]
async fn get_play_history(game_dir_name: String) -> Result<models::PlayHistoryStats, String> {
    history::get_stats(&game_dir_name).await
}

#[tauri::command]
async fn get_accounts() -> Result<Vec<models::AccountInfo>, String> {
    Ok(auth::get_accounts().await)
}

#[tauri::command]
async fn get_active_account() -> Result<Option<models::AccountInfo>, String> {
    Ok(auth::get_active_account().await)
}

#[tauri::command]
async fn set_active_account(uuid: String) -> Result<(), String> {
    auth::set_active_account(&uuid).await
}

#[tauri::command]
async fn remove_account(uuid: String) -> Result<(), String> {
    auth::remove_account(&uuid).await
}

#[tauri::command]
async fn start_device_code_flow(
    app_state: tauri::State<'_, state::AppState>,
) -> Result<models::DeviceCodeInfo, String> {
    use std::sync::atomic::Ordering;
    app_state.auth_poll_cancelled.store(false, Ordering::SeqCst);
    auth::microsoft::start_device_code_flow(&app_state.http_client).await
}

#[tauri::command]
async fn poll_device_code(
    device_code: String,
    interval: u32,
    app_state: tauri::State<'_, state::AppState>,
) -> Result<models::AuthResult, String> {
    let state = (*app_state).clone();
    auth::microsoft::poll_until_done(&state.http_client, &device_code, interval, &state).await
}

#[tauri::command]
async fn cancel_device_code_flow(
    app_state: tauri::State<'_, state::AppState>,
) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    app_state.auth_poll_cancelled.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
async fn refresh_token(
    uuid: String,
    app_state: tauri::State<'_, state::AppState>,
) -> Result<bool, String> {
    auth::microsoft::refresh_token(&app_state.http_client, &uuid).await
}

#[tauri::command]
async fn add_offline_account(username: String) -> Result<models::AuthResult, String> {
    auth::offline::add_offline_account(&username).await
}

fn build_updater(app: &tauri::AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    use tauri_plugin_updater::UpdaterExt;
    if let Some(url) = &build_config::get().server.updates_url {
        let endpoint: url::Url = url.parse().map_err(|e| format!("invalid updates_url: {e}"))?;
        app.updater_builder()
            .endpoints(vec![endpoint])
            .map_err(|e| e.to_string())?
            .build()
            .map_err(|e| e.to_string())
    } else {
        app.updater().map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn check_launcher_update(app: tauri::AppHandle) -> Result<Option<models::UpdateInfo>, String> {
    let updater = build_updater(&app)?;
    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => Ok(Some(models::UpdateInfo {
            version: update.version,
            body: update.body,
        })),
        None => Ok(None),
    }
}

#[tauri::command]
async fn download_and_install_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = build_updater(&app)?;
    if let Some(update) = updater.check().await.map_err(|e| e.to_string())? {
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| e.to_string())?;
        app.restart();
    }
    Ok(())
}

#[tauri::command]
fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn enable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    app.autolaunch().enable().map_err(|e| e.to_string())
}

#[tauri::command]
fn disable_autostart(app: tauri::AppHandle) -> Result<(), String> {
    app.autolaunch().disable().map_err(|e| e.to_string())
}

#[tauri::command]
async fn launch_instance(
    game_dir_name: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, state::AppState>,
) -> Result<(), String> {
    let state_clone = (*state).clone();
    let app = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        if let Err(e) = game::launch(&game_dir_name, app.clone(), &state_clone).await {
            game::emit_error(&app, e);
        }
    });

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            get_launcher_build_config,
            get_launcher_config,
            save_launcher_config,
            get_instance_local_config,
            save_instance_local_config,
            fetch_news,
            invalidate_news_cache,
            fetch_instances,
            fetch_instance_manifest,
            detect_java,
            download_java,
            launch_instance,
            get_play_history,
            get_accounts,
            get_active_account,
            set_active_account,
            remove_account,
            start_device_code_flow,
            poll_device_code,
            cancel_device_code_flow,
            refresh_token,
            add_offline_account,
            check_launcher_update,
            download_and_install_update,
            is_autostart_enabled,
            enable_autostart,
            disable_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
