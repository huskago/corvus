use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::AppHandle;

use crate::game::{emit_status, vanilla};
use crate::models::ModLoader;

pub struct LoaderResult {
    pub main_class: Option<String>,
    pub extra_jars: Vec<PathBuf>,
}

pub async fn prepare(
    client: &reqwest::Client,
    instance_dir: &Path,
    mc_version: &str,
    loader: &ModLoader,
    loader_version: &str,
    java_path: &str,
    app: &AppHandle,
) -> Result<LoaderResult, String> {
    match loader {
        ModLoader::Vanilla => Ok(LoaderResult {
            main_class: None,
            extra_jars: vec![],
        }),
        ModLoader::Fabric => {
            fabric_quilt(
                client,
                instance_dir,
                mc_version,
                loader_version,
                "https://meta.fabricmc.net",
                app,
            )
            .await
        }
        ModLoader::Quilt => {
            fabric_quilt(
                client,
                instance_dir,
                mc_version,
                loader_version,
                "https://meta.quiltmc.org",
                app,
            )
            .await
        }
        ModLoader::Forge => {
            forge(
                client,
                instance_dir,
                mc_version,
                loader_version,
                java_path,
                false,
                app,
            )
            .await
        }
        ModLoader::NeoForge => {
            forge(
                client,
                instance_dir,
                mc_version,
                loader_version,
                java_path,
                true,
                app,
            )
            .await
        }
    }
}

// Fabric & Quilt, same API Meta

#[derive(Deserialize)]
struct FabricProfile {
    #[serde(rename = "mainClass")]
    main_class: String,
    libraries: Vec<FabricLib>,
}

#[derive(Deserialize)]
struct FabricLib {
    name: String,
    url: String,
    sha1: Option<String>,
}

async fn fabric_quilt(
    client: &reqwest::Client,
    instance_dir: &Path,
    mc_version: &str,
    loader_version: &str,
    meta_base: &str,
    app: &AppHandle,
) -> Result<LoaderResult, String> {
    let api_path = if meta_base.contains("quiltmc") {
        "v3"
    } else {
        "v2"
    };

    let profile_url = format!(
        "{meta_base}/{api_path}/versions/loader/{mc_version}/{loader_version}/profile/json",
    );

    emit_status(app, format!("Downloading the loader profile..."));

    let profile: FabricProfile = client
        .get(&profile_url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch loader profile: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid loader profile JSON: {e}"))?;

    let libs_dir = instance_dir.join("libraries");
    let mut extra_jars = Vec::new();

    for lib in &profile.libraries {
        let Some(rel_path) = maven_path(&lib.name) else {
            continue;
        };
        let dest = libs_dir.join(&rel_path);

        let base = lib.url.trim_end_matches('/');
        let url = format!("{base}/{rel_path}");

        if let Some(ref sha1) = lib.sha1 {
            vanilla::download_verified(client, &url, &dest, sha1).await?;
        } else {
            if !dest.exists() {
                if let Some(parent) = dest.parent() {
                    tokio::fs::create_dir_all(parent).await.ok();
                }
                download_no_verify(client, &url, &dest).await?;
            }
        }

        extra_jars.push(dest);
    }

    Ok(LoaderResult {
        main_class: Some(profile.main_class),
        extra_jars,
    })
}

fn maven_path(coord: &str) -> Option<String> {
    let parts: Vec<&str> = coord.splitn(3, ':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    Some(format!(
        "{}/{artifact}/{version}/{artifact}-{version}.jar",
        group.replace('.', "/")
    ))
}

async fn download_no_verify(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("DL {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} - {url}", resp.status()));
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Created {}: {e}", dest.display()))?;
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("{e}"))? {
        file.write_all(&chunk).await.map_err(|e| format!("{e}"))?;
    }
    Ok(())
}

// Forge & Neoforge, with official installer

async fn forge(
    client: &reqwest::Client,
    instance_dir: &Path,
    mc_version: &str,
    loader_version: &str,
    java_path: &str,
    is_neoforge: bool,
    app: &AppHandle,
) -> Result<LoaderResult, String> {
    let installer_url = if is_neoforge {
        format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge\
            /{loader_version}/neoforge-{loader_version}-installer.jar"
        )
    } else {
        format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge\
             /{mc_version}-{loader_version}/forge-{mc_version}-{loader_version}-installer.jar"
        )
    };

    let installer_name = if is_neoforge {
        format!("neoforge-{loader_version}-installer.jar")
    } else {
        format!("forge-{mc_version}-{loader_version}-installer.jar")
    };

    let installer_path = instance_dir.join(&installer_name);

    if !installer_path.exists() {
        emit_status(
            app,
            format!(
                "Downloading the installer {}...",
                if is_neoforge { "NeoForge" } else { "Forge" }
            ),
        );
        download_no_verify(client, &installer_url, &installer_path).await?;
    }

    emit_status(
        app,
        "Running the installer (this may take a few minutes)...",
    );

    let output = tokio::process::Command::new(java_path)
        .arg("-jar")
        .arg(&installer_path)
        .arg("--installClient")
        .arg(instance_dir)
        .current_dir(instance_dir)
        .output()
        .await
        .map_err(|e| format!("Installer launch failed: {e}"))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let details = format!("{stdout}{stderr}").trim().to_string();
        let code = output.status.code().unwrap_or(-1);
        let msg = if details.is_empty() {
            format!("Installer failed (code {code}), no output captured. Check that Java {java_path} is valid and that the installer URL is correct.")
        } else {
            format!("Installer failed (code {code}):\n{details}")
        };
        return Err(msg);
    }

    tokio::fs::remove_file(&installer_path).await.ok();

    let version_json = find_installed_version_json(instance_dir, mc_version, is_neoforge)
        .await
        .ok_or_else(|| "JSON version not found after installation".to_string())?;

    parse_installed_version(instance_dir, &version_json)
}

async fn find_installed_version_json(
    instance_dir: &Path,
    mc_version: &str,
    is_neoforge: bool,
) -> Option<PathBuf> {
    let versions_dir = instance_dir.join("versions");
    let keyword = if is_neoforge { "neoforge" } else { "forge" };

    let mut entries = tokio::fs::read_dir(&versions_dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        let matches =
            name.to_lowercase().contains(keyword) && (is_neoforge || name.contains(mc_version));
        if matches {
            let json = entry.path().join(format!("{name}.json"));
            if json.exists() {
                return Some(json);
            }
        }
    }
    None
}

fn parse_installed_version(
    instance_dir: &Path,
    json_path: &PathBuf,
) -> Result<LoaderResult, String> {
    let content =
        std::fs::read_to_string(json_path).map_err(|e| format!("Read JSON Forge version: {e}"))?;

    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {e}"))?;

    let main_class = json["mainClass"].as_str().map(String::from);

    let libs_dir = instance_dir.join("libraries");
    let mut extra_jars = Vec::new();

    if let Some(libs) = json["libraries"].as_array() {
        for lib in libs {
            if let Some(path) = lib["downloads"]["artifact"]["path"].as_str() {
                let jar = libs_dir.join(path);
                if jar.exists() {
                    extra_jars.push(jar);
                }
            } else if let Some(name) = lib["name"].as_str() {
                if let Some(rel) = maven_path(name) {
                    let jar = libs_dir.join(&rel);
                    if jar.exists() {
                        extra_jars.push(jar);
                    }
                }
            }
        }
    }

    Ok(LoaderResult {
        main_class,
        extra_jars,
    })
}
