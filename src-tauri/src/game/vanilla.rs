use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::game::{emit_progress, emit_status, ProgressPayload};

const VERSION_MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json";
const ASSETS_BASE_URL: &str = "https://resources.download.minecraft.net";
const PARALLEL_DOWNLOADS: usize = 6;

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
}

#[derive(Deserialize, Serialize)]
pub struct VersionJson {
    #[serde(rename = "mainClass")]
    pub main_class: String,
    pub arguments: Option<RawArguments>,
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: Option<String>,
    #[serde(rename = "assetIndex")]
    pub asset_index: AssetIndexRef,
    pub downloads: VersionDownloads,
    pub libraries: Vec<Library>,
}

#[derive(Deserialize, Serialize)]
pub struct RawArguments {
    pub game: Vec<serde_json::Value>,
    pub jvm: Vec<serde_json::Value>,
}

#[derive(Deserialize, Serialize)]
pub struct AssetIndexRef {
    pub id: String,
    pub url: String,
    pub sha1: String,
}

#[derive(Deserialize, Serialize)]
pub struct VersionDownloads {
    pub client: FileDownload,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct FileDownload {
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize, Serialize)]
pub struct Library {
    pub name: String,
    pub downloads: LibraryDownloads,
    #[serde(default)]
    pub rules: Vec<LibraryRule>,
}

#[derive(Deserialize, Serialize)]
pub struct LibraryDownloads {
    pub artifact: Option<LibraryArtifact>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct LibraryArtifact {
    pub path: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize, Serialize)]
pub struct LibraryRule {
    pub action: String,
    pub os: Option<OsRule>,
}

#[derive(Deserialize, Serialize)]
pub struct OsRule {
    pub name: Option<String>,
}

#[derive(Deserialize)]
struct AssetIndex {
    objects: HashMap<String, AssetObject>,
}

#[derive(Deserialize)]
struct AssetObject {
    hash: String,
}

pub async fn sha1_of_file(path: &Path) -> Option<String> {
    use sha1::{Digest, Sha1};
    use std::io::Read;

    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path).ok()?;
        let mut hasher = Sha1::new();

        let mut buf = [0u8; 65536];
        loop {
            let n = file.read(&mut buf).ok()?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }

        let result = hasher.finalize();
        Some(
            result
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
        )
    })
    .await
    .ok()
    .flatten()
}

pub async fn file_is_valid(path: &Path, expected_sha1: &str) -> bool {
    if !path.exists() {
        return false;
    }
    sha1_of_file(path)
        .await
        .map_or(false, |actual| actual == expected_sha1)
}

pub async fn download_verified(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: &str,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    if url.is_empty() || !url.starts_with("http") {
        return Err(format!("invalid download URL '{url}'"));
    }

    if file_is_valid(dest, expected_sha1).await {
        return Ok(());
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Folder creation: {e}"))?;
    }

    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} - {url}", response.status()));
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("File creation: {e}"))?;

    while let Some(chunk) = response.chunk().await.map_err(|e| format!("{e}"))? {
        file.write_all(&chunk).await.map_err(|e| format!("{e}"))?;
    }
    drop(file);

    if !expected_sha1.is_empty() && !file_is_valid(dest, expected_sha1).await {
        tokio::fs::remove_file(dest).await.ok();
        return Err(format!("SHA1 mismatch after download: {url|}"));
    }

    Ok(())
}

pub async fn ensure_vanilla_files(
    client: &reqwest::Client,
    instance_dir: &Path,
    mc_version: &str,
    app: &AppHandle,
) -> Result<VersionJson, String> {
    emit_status(app, "Retrieving the Mojang manifest...");
    let manifest: VersionManifest = client
        .get(VERSION_MANIFEST_URL)
        .send()
        .await
        .map_err(|e| format!("Mojang Manifest is unavailable: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid JSON manifest: {e}"))?;

    let entry = manifest
        .versions
        .into_iter()
        .find(|v| v.id == mc_version)
        .ok_or_else(|| format!("Version {mc_version} not found in the Mojang manifest"))?;

    let versions_dir = instance_dir.join("versions").join(mc_version);
    let version_json_path = versions_dir.join(format!("{mc_version}.json"));

    emit_status(app, format!("Loading Minecraft metadata {mc_version}..."));

    let raw_json = client
        .get(&entry.url)
        .send()
        .await
        .map_err(|e| format!("JSON version not available: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Read JSON version: {e}"))?;

    tokio::fs::create_dir_all(&versions_dir)
        .await
        .map_err(|e| format!("Create version folder: {e}"))?;
    tokio::fs::write(&version_json_path, &raw_json)
        .await
        .map_err(|e| format!("Save as JSON: {e}"))?;

    let version_json: VersionJson =
        serde_json::from_str(&raw_json).map_err(|e| format!("Invalid JSON version: {e}"))?;

    let client_jar = versions_dir.join(format!("{mc_version}.jar"));
    emit_status(app, "Checking the client JAR...");
    download_verified(
        client,
        &version_json.downloads.client.url,
        &client_jar,
        &version_json.downloads.client.sha1,
    )
    .await?;

    emit_status(app, "Checking libraries...");
    let libs_dir = instance_dir.join("libraries");
    download_libraries(client, &version_json.libraries, &libs_dir, app).await?;

    emit_status(app, "Checking assets...");
    let assets_dir = instance_dir.join("assets");
    download_assets(client, &version_json.asset_index, &assets_dir, app).await?;

    Ok(version_json)
}

async fn download_libraries(
    client: &reqwest::Client,
    libraries: &[Library],
    libs_dir: &Path,
    app: &AppHandle,
) -> Result<(), String> {
    let to_download: Vec<(String, String, PathBuf)> = libraries
        .iter()
        .filter(|lib| lib_applies_to_current_os(lib))
        .filter_map(|lib| {
            let art = lib.downloads.artifact.as_ref()?;
            if art.url.is_empty() || art.sha1.is_empty() {
                eprintln!("[SKIP] lib {}, url={:?} sha1={:?}", lib.name, art.url, art.sha1);
                return None;
            }
            Some((art.url.clone(), art.sha1.clone(), libs_dir.join(&art.path)))
        })
        .collect();

    download_parallel(client, to_download, "libraries", app).await
}

pub fn lib_applies_to_current_os(lib: &Library) -> bool {
    if lib.rules.is_empty() {
        return true;
    }

    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    };

    let mut allowed = false;
    for rule in &lib.rules {
        let os_matches = rule
            .os
            .as_ref()
            .and_then(|os| os.name.as_deref())
            .map_or(true, |name| name == current_os);

        if os_matches {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

pub fn collect_library_paths(version_json: &VersionJson, libs_dir: &Path) -> Vec<PathBuf> {
    version_json
        .libraries
        .iter()
        .filter(|lib| lib_applies_to_current_os(lib))
        .filter_map(|lib| {
            let artifact = lib.downloads.artifact.as_ref()?;
            Some(libs_dir.join(&artifact.path))
        })
        .collect()
}

async fn download_assets(
    client: &reqwest::Client,
    asset_ref: &AssetIndexRef,
    assets_dir: &Path,
    app: &AppHandle,
) -> Result<(), String> {
    let index_path = assets_dir
        .join("indexes")
        .join(format!("{}.json", asset_ref.id));
    download_verified(client, &asset_ref.url, &index_path, &asset_ref.sha1).await?;

    let content = tokio::fs::read_to_string(&index_path)
        .await
        .map_err(|e| format!("Reading index assets: {e}"))?;
    let index: AssetIndex =
        serde_json::from_str(&content).map_err(|e| format!("Invalid index: {e}"))?;

    let objects_dir = assets_dir.join("objects");

    let to_download: Vec<(String, String, PathBuf)> = index
        .objects
        .into_values()
        .filter(|obj| obj.hash.len() >= 2)
        .map(|obj| {
            let prefix = obj.hash[..2].to_string();
            let url = format!("{ASSETS_BASE_URL}/{prefix}/{}", obj.hash);
            let dest = objects_dir.join(&prefix).join(&obj.hash);
            (url, obj.hash, dest)
        })
        .collect();

    download_parallel(client, to_download, "assets", app).await
}

async fn download_counted(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: &str,
) -> Result<u64, String> {
    use tokio::io::AsyncWriteExt;

    if url.is_empty() || !url.starts_with("http") {
        return Err(format!("invalid download URL '{url}'"));
    }

    if file_is_valid(dest, expected_sha1).await {
        return Ok(0);
    }

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Folder creation: {e}"))?;
    }

    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Network: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {} - {url}", response.status()));
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("File creation: {e}"))?;

    let mut total = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("{e}"))? {
        total += chunk.len() as u64;
        file.write_all(&chunk).await.map_err(|e| format!("{e}"))?;
    }
    drop(file);

    if !expected_sha1.is_empty() && !file_is_valid(dest, expected_sha1).await {
        tokio::fs::remove_file(dest).await.ok();
        return Err(format!("SHA1 mismatch after download: {url|}"));
    }

    Ok(total)
}

async fn download_parallel(
    client: &reqwest::Client,
    files: Vec<(String, String, PathBuf)>,
    step_name: &str,
    app: &AppHandle,
) -> Result<(), String> {
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    let total = files.len() as u32;
    if total == 0 {
        return Ok(());
    }

    let semaphore = Arc::new(tokio::sync::Semaphore::new(PARALLEL_DOWNLOADS));
    let client = client.clone();
    let files_done = Arc::new(AtomicU32::new(0));
    let bytes_done = Arc::new(AtomicU64::new(0));
    let start = std::time::Instant::now();
    let step = step_name.to_string();

    let mut join_set = tokio::task::JoinSet::new();

    for (url, sha1, dest) in files {
        let client = client.clone();
        let semaphore = Arc::clone(&semaphore);
        let files_done = Arc::clone(&files_done);
        let bytes_done = Arc::clone(&bytes_done);

        join_set.spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            let n = download_counted(&client, &url, &dest, &sha1).await?;
            bytes_done.fetch_add(n, Ordering::Relaxed);
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
                        step: step.clone(),
                        files_done: done,
                        files_total: total,
                        bytes_done: bytes,
                        speed_bps: speed,
                    },
                );
            }
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(format!("Download task failed: {e}")),
        }
    }

    Ok(())
}
