use std::path::PathBuf;

use serde::Deserialize;

use crate::config::base_dir;
use crate::models::JavaInfo;

pub fn required_java_major(mc_version: &str) -> u32 {
    let parts: Vec<u32> = mc_version
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    let (major, minor) = match parts.as_slice() {
        [maj, min, ..] => (*maj, *min),
        [maj] => (*maj, 0),
        _ => return 21,
    };

    if major > 1 || (major == 1 && minor >= 21) {
        21
    } else if major == 1 && minor >= 17 {
        17
    } else {
        8
    }
}

fn java_install_dir(major: u32) -> PathBuf {
    base_dir().join("java").join(format!("java-{major}"))
}

pub async fn detect_java(java_path_override: Option<&str>, mc_version: &str) -> Option<JavaInfo> {
    let required = required_java_major(mc_version);

    // Manual path
    if let Some(path) = java_path_override {
        if !path.is_empty() {
            if let Some(info) = probe_java(path).await {
                if info.version == required {
                    return Some(info);
                }
            }
        }
    }

    // JAVA_HOME
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let bin_name = if cfg!(target_os = "windows") {
            "java.exe"
        } else {
            "java"
        };
        let bin = PathBuf::from(&java_home).join("bin").join(bin_name);
        if let Some(path_str) = bin.to_str() {
            if let Some(info) = probe_java(path_str).await {
                if info.version == required {
                    return Some(info);
                }
            }
        }
    }

    // `java` in the PATH
    let java_cmd = if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    };
    if let Some(info) = probe_java(java_cmd).await {
        if info.version == required {
            return Some(info);
        }
    }

    // Java installed via the launcher
    find_launcher_java(required).await
}

async fn probe_java(path: &str) -> Option<JavaInfo> {
    let path_owned = path.to_string();

    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(&path_owned)
            .arg("-version")
            .output()
    })
    .await
    .ok()?
    .ok()?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = if stderr.is_empty() { &stdout } else { &stderr };

    let version = parse_java_version(text)?;

    Some(JavaInfo {
        path: path.to_string(),
        version,
    })
}

async fn find_launcher_java(major: u32) -> Option<JavaInfo> {
    let dir = java_install_dir(major);
    if !dir.exists() {
        return None;
    }

    let mut entries = tokio::fs::read_dir(&dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let bin_name = if cfg!(target_os = "windows") {
            "java.exe"
        } else {
            "java"
        };
        let java_bin = entry_path.join("bin").join(bin_name);

        if java_bin.exists() {
            if let Some(bin_str) = java_bin.to_str() {
                if let Some(info) = probe_java(bin_str).await {
                    if info.version == major {
                        return Some(info);
                    }
                }
            }
        }
    }

    None
}

fn parse_java_version(output: &str) -> Option<u32> {
    let start = output.find('"')? + 1;
    let end = start + output[start..].find('"')?;
    let version_str = &output[start..end];

    let first = version_str.split('.').next()?;

    if first == "1" {
        version_str.split('.').nth(1)?.parse().ok()
    } else {
        first.parse().ok()
    }
}

#[derive(Deserialize)]
struct ZuluPackage {
    download_url: String,
    name: String,
}

pub async fn download_java(client: &reqwest::Client, mc_version: &str) -> Result<String, String> {
    let java_major = required_java_major(mc_version);

    if let Some(info) = find_launcher_java(java_major).await {
        return Ok(info.path);
    }

    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x86_64"
    };

    let api_url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/\
        ?java_version={java_major}&os={os}&arch={arch}\
        &package_type=jdk&release_status=ga&latest=true"
    );

    let packages: Vec<ZuluPackage> = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| format!("Azul API unavailable: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Invalid Azul API response: {e}"))?;

    let package = packages
        .into_iter()
        .next()
        .ok_or_else(|| format!("No Java JDK {java_major} found for {os}/{arch}"))?;

    let install_dir = java_install_dir(java_major);
    tokio::fs::create_dir_all(&install_dir)
        .await
        .map_err(|e| format!("Unable to create the Java folder: {e}"))?;

    let archive_path = install_dir.join(&package.name);
    download_file(client, &package.download_url, &archive_path).await?;

    let archive_clone = archive_path.clone();
    let extract_to = install_dir.clone();
    tokio::task::spawn_blocking(move || extract_archive(&archive_clone, &extract_to))
        .await
        .map_err(|e| format!("Extraction thread has crashed: {e}"))??;

    let _ = tokio::fs::remove_file(&archive_path).await;

    find_launcher_java(java_major)
        .await
        .map(|info| info.path)
        .ok_or_else(|| {
            "Java is installed, but the binary cannot be found after extraction".to_string()
        })
}

async fn download_file(client: &reqwest::Client, url: &str, dest: &PathBuf) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download error: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Download refused by the server: {}",
            response.status()
        ));
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Unable to create the file: {e}"))?;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Network read error: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Disk write error: {e}"))?;
    }

    Ok(())
}

fn extract_archive(archive: &PathBuf, dest: &PathBuf) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use zip::ZipArchive;
        let file =
            std::fs::File::open(archive).map_err(|e| format!("Unable to open ZIP file: {e}"))?;
        let mut zip = ZipArchive::new(file).map_err(|e| format!("Corrupted ZIP archive: {e}"))?;
        zip.extract(dest)
            .map_err(|e| format!("ZIP extraction failed: {e}"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        use flate2::read::GzDecoder;
        use tar::Archive;
        let file =
            std::fs::File::open(archive).map_err(|e| format!("Unable to open tar.gz file: {e}"))?;
        let decompressed = GzDecoder::new(file);
        let mut archive_tar = Archive::new(decompressed);
        archive_tar
            .unpack(dest)
            .map_err(|e| format!("tar.gz extraction failed: {e}"))
    }
}
