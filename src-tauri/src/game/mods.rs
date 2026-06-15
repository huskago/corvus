use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::AppHandle;
use url::Url;

use crate::game::vanilla::{download_verified, file_is_valid};
use crate::game::{emit_progress, emit_status, ProgressPayload};
use crate::models::{InstanceManifest, LocalInstanceConfig, ManifestFile, ModStatus};

fn resolve_download_url(update_url: &str, download_url: &str) -> Result<String, String> {
    if download_url.starts_with("http") {
        return Ok(download_url.to_string());
    }

    let base = Url::parse(update_url)
        .map_err(|e| format!("Invalid updateUrl: {e}"))?;

    base.join(download_url)
        .map_err(|e| format!("Unable to resolve the download URL “{download_url}”: {e}"))
        .map(|u| u.to_string())
}

pub async fn sync_manifest(
    client: &reqwest::Client,
    manifest: &InstanceManifest,
    instance_dir: &Path,
    update_url: &str,
    local_config: &LocalInstanceConfig,
    app: &AppHandle,
) -> Result<(), String> {
    let mods_dir = instance_dir.join("mods");
    let rp_dir = instance_dir.join("resourcepacks");
    let shaders_dir = instance_dir.join("shaderpacks");

    let mut all_files: Vec<(&ManifestFile, PathBuf)> = Vec::new();
    for f in &manifest.mods {
        all_files.push((f, mods_dir.clone()));
    }
    for f in &manifest.resource_packs {
        all_files.push((f, rp_dir.clone()));
    }
    for f in &manifest.shaders {
        all_files.push((f, shaders_dir.clone()));
    }

    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    let total = all_files.len() as u32;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(4));
    let client = client.clone();
    let files_done = Arc::new(AtomicU32::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let start = std::time::Instant::now();

    emit_status(app, "Synchronizing mods...");

    let mut join_set = tokio::task::JoinSet::new();

    for (file, dir) in all_files {
        let client = client.clone();
        let semaphore = Arc::clone(&semaphore);
        let files_done = Arc::clone(&files_done);
        let bytes_done = Arc::clone(&bytes_done);
        let file_size = file.size;
        let file = file.clone();
        let active = should_be_active(&file, local_config);
        let update_url = update_url.to_string();

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            sync_single_file(&client, &file, &dir, active, &update_url).await?;
            bytes_done.fetch_add(file_size, Ordering::Relaxed);
            let done = files_done.fetch_add(1, Ordering::Relaxed) + 1;
            Ok::<(u32, u64), String>((done, bytes_done.load(Ordering::Relaxed)))
        });
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok((done, bytes))) => {
                let elapsed = start.elapsed().as_secs_f64().max(0.001);
                let speed = (bytes as f64 / elapsed) as u64;
                emit_progress(
                    app,
                    ProgressPayload {
                        step: "mods".to_string(),
                        files_done: done,
                        files_total: total,
                        bytes_done: bytes,
                        speed_bps: speed,
                    },
                );
            }
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(format!("Mod task failed: {e}")),
        }
    }

    emit_status(app, "Synchronizing configuration files...");
    for extra in &manifest.extra_files {
        let dest = instance_dir.join(&extra.path);
        let url = resolve_download_url(update_url, &extra.download_url)?;
        download_verified(&client, &url, &dest, &extra.sha1).await?
    }

    Ok(())
}

fn should_be_active(file: &ManifestFile, config: &LocalInstanceConfig) -> bool {
    match file.status {
        ModStatus::Required => true,
        ModStatus::OptionalOn => !config.disabled_mods.contains(&file.name),
        ModStatus::OptionalOff => config.enabled_mods.contains(&file.name),
    }
}

async fn sync_single_file(
    client: &reqwest::Client,
    file: &ManifestFile,
    dir: &PathBuf,
    active: bool,
    update_url: &str,
) -> Result<(), String> {
    let active_path = dir.join(&file.name);
    let disabled_path = dir.join(format!("{}.disabled", file.name));

    let active_valid = file_is_valid(&active_path, &file.sha1).await;
    let disabled_valid = file_is_valid(&disabled_path, &file.sha1).await;

    if active && active_valid {
        return Ok(());
    }
    if !active && disabled_valid {
        return Ok(());
    }

    if active_valid && !active {
        return tokio::fs::rename(&active_path, &disabled_path)
            .await
            .map_err(|e| format!("Rename {}: {e}", file.name));
    }
    if disabled_valid && active {
        return tokio::fs::rename(&disabled_path, &active_path)
            .await
            .map_err(|e| format!("Rename {}: {e}", file.name));
    }

    let dest = if active { &active_path } else { &disabled_path };
    let url = resolve_download_url(update_url, &file.download_url)?;
    download_verified(client, &url, dest, &file.sha1).await
}
