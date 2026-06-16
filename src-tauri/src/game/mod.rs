pub mod loader;
pub mod mods;
pub mod process;
pub mod vanilla;

use tauri::{AppHandle, Emitter};

use crate::models::InstanceInfo;
use crate::{config, history, instances, state::AppState};

#[derive(serde::Serialize, Clone)]
pub struct StatusPayload {
    pub message: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ProgressPayload {
    pub step: String,
    pub files_done: u32,
    pub files_total: u32,
    pub bytes_done: u64,
    pub speed_bps: u64,
}

#[derive(serde::Serialize, Clone)]
pub struct LogPayload {
    pub line: String,
}

#[derive(serde::Serialize, Clone)]
pub struct ErrorPayload {
    pub message: String,
}

pub fn emit_status(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit(
        "launch:status",
        StatusPayload {
            message: message.into(),
        },
    );
}

pub fn emit_progress(app: &AppHandle, payload: ProgressPayload) {
    let _ = app.emit("launch:progress", payload);
}

pub fn emit_log(app: &AppHandle, line: impl Into<String>) {
    let _ = app.emit("launch:log", LogPayload { line: line.into() });
}

pub fn emit_done(app: &AppHandle) {
    let _ = app.emit("launch:done", ());
}

pub fn emit_error(app: &AppHandle, message: impl Into<String>) {
    let _ = app.emit(
        "launch:error",
        ErrorPayload {
            message: message.into(),
        },
    );
}

pub async fn launch(game_dir_name: &str, app: AppHandle, state: &AppState) -> Result<(), String> {
    {
        let mut running = state.running_instance.lock().await;
        if running.is_some() {
            return Err(format!(
                "Instance '{}' is already running",
                running.as_deref().unwrap_or("unknown")
            ));
        }
        *running = Some(game_dir_name.to_string());
    }

    let result = async {
        let instances = instances::get_instances(state).await?;
        let instance = instances
            .into_iter()
            .find(|i| i.game_dir_name == game_dir_name)
            .ok_or_else(|| format!("Instance '{}' not found", game_dir_name))?;

        if instance.maintenance {
            return Err("This service is currently undergoing maintenance.".to_string());
        }

        do_launch(game_dir_name, &app, state, &instance).await
    }
    .await;

    {
        let mut running = state.running_instance.lock().await;
        *running = Some(game_dir_name.to_string());
    }

    result
}

async fn do_launch(
    game_dir_name: &str,
    app: &AppHandle,
    state: &AppState,
    instance: &InstanceInfo,
) -> Result<(), String> {
    let instance_dir = config::instance_dir(game_dir_name);
    let local_config = config::read_instance_config(game_dir_name).await?;

    emit_status(
        app,
        format!("Checking Minecraft {}...", instance.mc_version),
    );
    let version_json =
        vanilla::ensure_vanilla_files(&state.http_client, &instance_dir, &instance.mc_version, app)
            .await?;

    emit_status(app, "Synchronising mods...");
    match instances::fetch_manifest(&state.http_client, &instance.update_url).await {
        Ok(manifest) => {
            mods::sync_manifest(
                &state.http_client,
                &manifest,
                &instance_dir,
                &instance.update_url,
                &local_config,
                app,
            )
            .await?;
        }
        Err(e) => {
            emit_status(app, format!("Manifest unavailable, skipping mods sync ({e})"));
        }
    }

    emit_status(app, "Search for Java…");
    let java_override = local_config
        .java_path
        .is_empty()
        .then_some(None)
        .unwrap_or(Some(local_config.java_path.as_str()));

    let java_path = match crate::java::detect_java(java_override, &instance.mc_version).await {
        Some(info) => info.path,
        None => {
            emit_status(app, "Java not found, downloading Azul Zulu...");
            crate::java::download_java(&state.http_client, &instance.mc_version).await?
        }
    };

    emit_status(app, "Preparing the mod loader...");
    let loader_result = loader::prepare(
        &state.http_client,
        &instance_dir,
        &instance.mc_version,
        &instance.loader,
        &instance.loader_version,
        &java_path,
        app,
    )
    .await?;

    let (account_name, account_uuid, mc_access_token) =
        match crate::auth::get_launch_credentials().await {
            Some((_mode, name, uuid, token)) => (name, uuid, token),
            None => return Err("No active accounts. Log in via the Profile page.".to_string()),
        };

    let start_time = history::current_time_ms();

    process::spawn_minecraft(
        process::LaunchParams {
            java_path: &java_path,
            instance_dir: &instance_dir,
            mc_version: &instance.mc_version,
            version_json: &version_json,
            main_class_override: loader_result.main_class.as_deref(),
            extra_classpath: &loader_result.extra_jars,
            account_name: &account_name,
            account_uuid: &account_uuid,
            mc_access_token: &mc_access_token,
            server_ip: instance.server_ip.as_deref(),
            local_config: &local_config,
        },
        app,
    )
    .await?;

    history::record_session(
        game_dir_name,
        start_time,
        history::current_time_ms(),
        &account_uuid,
        &account_name,
    )
    .await;

    emit_done(app);
    Ok(())
}
