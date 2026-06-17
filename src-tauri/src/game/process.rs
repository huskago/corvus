use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::AppHandle;

use crate::game::vanilla;
use crate::game::{emit_log, emit_status};
use crate::models::LocalInstanceConfig;

pub struct LaunchParams<'a> {
    pub java_path: &'a str,
    pub instance_dir: &'a Path,
    pub mc_version: &'a str,
    pub version_json: &'a vanilla::VersionJson,
    pub main_class_override: Option<&'a str>,
    pub extra_classpath: &'a [PathBuf],
    pub extra_jvm_args: &'a [String],
    pub extra_game_args: &'a [String],
    pub account_name: &'a str,
    pub account_uuid: &'a str,
    pub mc_access_token: &'a str,
    pub server_ip: Option<&'a str>,
    pub local_config: &'a LocalInstanceConfig,
}

pub async fn spawn_minecraft(
    params: LaunchParams<'_>,
    app: &AppHandle,
    kill_flag: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    let libs_dir = params.instance_dir.join("libraries");
    let assets_dir = params.instance_dir.join("assets");
    let natives_dir = params
        .instance_dir
        .join("versions")
        .join(params.mc_version)
        .join("natives");
    let client_jar = params
        .instance_dir
        .join("versions")
        .join(params.mc_version)
        .join(format!("{}.jar", params.mc_version));

    tokio::fs::create_dir_all(&natives_dir).await.ok();

    let cp_sep = if cfg!(target_os = "windows") {
        ";"
    } else {
        ":"
    };

    let mut cp_jars = vanilla::collect_library_paths(params.version_json, &libs_dir);
    cp_jars.extend_from_slice(params.extra_classpath);
    cp_jars.push(client_jar);

    // Deduplicate while preserving order (vanilla first, loader overrides)
    let mut seen = std::collections::HashSet::new();
    cp_jars.retain(|p| seen.insert(p.clone()));

    let classpath = cp_jars
        .iter()
        .filter_map(|p| p.to_str())
        .collect::<Vec<_>>()
        .join(cp_sep);

    let subs: HashMap<String, String> = [
        (
            "natives_directory",
            natives_dir.to_str().unwrap_or("").to_string(),
        ),
        ("launcher_name", "Corvus".to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
        ("classpath", classpath.clone()),
        ("auth_player_name", params.account_name.to_string()),
        ("version_name", params.mc_version.to_string()),
        (
            "game_directory",
            params.instance_dir.to_str().unwrap_or("").to_string(),
        ),
        ("assets_root", assets_dir.to_str().unwrap_or("").to_string()),
        (
            "assets_index_name",
            params.version_json.asset_index.id.clone(),
        ),
        ("auth_uuid", params.account_uuid.to_string()),
        ("auth_access_token", params.mc_access_token.to_string()),
        ("clientid", "0".to_string()),
        ("auth_xuid", "0".to_string()),
        ("user_type", "msa".to_string()),
        ("version_type", "release".to_string()),
        ("user_properties", "{}".to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();

    let mut args: Vec<String> = Vec::new();

    args.push(format!("-Xms{}M", params.local_config.min_ram));
    args.push(format!("-Xmx{}M", params.local_config.max_ram));

    if params.local_config.optimized_args {
        args.extend(
            [
                "-XX:+UseG1GC",
                "-XX:+ParallelRefProcEnabled",
                "-XX:MaxGCPauseMillis=200",
                "-XX:+UnlockExperimentalVMOptions",
                "-XX:+DisableExplicitGC",
                "-XX:+AlwaysPreTouch",
                "-XX:G1NewSizePercent=30",
                "-XX:G1MaxNewSizePercent=40",
                "-XX:G1HeapRegionSize=8M",
                "-XX:G1ReservePercent=20",
                "-XX:G1HeapWastePercent=5",
                "-XX:G1MixedGCCountTarget=4",
            ]
            .map(String::from),
        );
    }

    args.extend(build_jvm_args(params.version_json, &subs));
    args.extend(params.extra_jvm_args.iter().cloned());

    if !params.local_config.jvm_args.is_empty() {
        args.extend(
            params
                .local_config
                .jvm_args
                .split_whitespace()
                .map(String::from),
        );
    }

    args.push(
        params
            .main_class_override
            .unwrap_or(&params.version_json.main_class)
            .to_string(),
    );

    args.extend(build_game_args(params.version_json, &subs));
    args.extend(params.extra_game_args.iter().cloned());

    if params.local_config.resolution_width > 0 && params.local_config.resolution_height > 0 {
        args.extend([
            "--width".to_string(),
            params.local_config.resolution_width.to_string(),
            "--height".to_string(),
            params.local_config.resolution_height.to_string(),
        ]);
    }

    if params.local_config.auto_connect_server {
        if let Some(ip) = params.server_ip {
            let mut parts = ip.splitn(2, ':');
            let host = parts.next().unwrap_or(ip);
            let port = parts.next().unwrap_or("25565");
            args.extend([
                "--server".to_string(),
                host.to_string(),
                "--port".to_string(),
                port.to_string(),
            ]);
        }
    }

    emit_status(app, format!("Launching Minecraft {}...", params.mc_version));
    emit_log(app, format!("Java: {}", params.java_path));
    emit_log(app, format!("Args: {}", args.join(" ")));

    let mut command = tokio::process::Command::new(params.java_path);
    command
        .args(&args)
        .current_dir(params.instance_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let mut child = command
        .spawn()
        .map_err(|e| format!("Unable to launch Java ({}): {e}", params.java_path))?;

    if let Some(stdout) = child.stdout.take() {
        let app_cl = app.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                emit_log(&app_cl, line);
            }
        });
    }

    let stderr_lines = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<String>::new()));
    if let Some(stderr) = child.stderr.take() {
        let app_cl = app.clone();
        let lines_cl = stderr_lines.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                emit_log(&app_cl, line.clone());
                lines_cl.lock().await.push(line);
            }
        });
    }

    loop {
        if kill_flag.load(std::sync::atomic::Ordering::Relaxed) {
            child.kill().await.ok();
            child.wait().await.ok();
            emit_status(app, "Stopped.");
            return Ok(());
        }
        match child.try_wait().map_err(|e| format!("Java process error: {e}"))? {
            Some(status) => {
                if !status.success() {
                    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                    let captured = stderr_lines.lock().await;
                    let last_lines: Vec<&str> =
                        captured.iter().take(15).map(|s| s.as_str()).collect();
                    let code = status.code().unwrap_or(-1);
                    return Err(format!(
                        "Java exited with code {code}:\n{}",
                        if last_lines.is_empty() {
                            "(no output captured)".to_string()
                        } else {
                            last_lines.join("\n")
                        }
                    ));
                }
                break;
            }
            None => tokio::time::sleep(tokio::time::Duration::from_millis(100)).await,
        }
    }

    Ok(())
}

fn sub(s: &str, subs: &HashMap<String, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in subs {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

fn build_game_args(vj: &vanilla::VersionJson, subs: &HashMap<String, String>) -> Vec<String> {
    if let Some(ref args) = vj.arguments {
        args.game
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| sub(s, subs))
            .collect()
    } else if let Some(ref mc_args) = vj.minecraft_arguments {
        mc_args.split_whitespace().map(|s| sub(s, subs)).collect()
    } else {
        Vec::new()
    }
}

fn build_jvm_args(vj: &vanilla::VersionJson, subs: &HashMap<String, String>) -> Vec<String> {
    if let Some(ref args) = vj.arguments {
        args.jvm
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| sub(s, subs))
            .collect()
    } else {
        vec![
            format!(
                "-Djava.library.path={}",
                subs.get("natives_directory")
                    .map(|s| s.as_str())
                    .unwrap_or("")
            ),
            "-cp".to_string(),
            subs.get("classpath").cloned().unwrap_or_default(),
        ]
    }
}
