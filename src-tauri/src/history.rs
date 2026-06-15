use crate::config::instance_dir;
use crate::models::{PlayHistoryStats, PlaySession};

const MAX_SESSIONS: usize = 100;

fn history_path(game_dir_name: &str) -> std::path::PathBuf {
    instance_dir(game_dir_name).join("play_history.json")
}

pub fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn load_sessions(game_dir_name: &str) -> Vec<PlaySession> {
    let path = history_path(game_dir_name);
    if !path.exists() {
        return Vec::new();
    }
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

async fn save_sessions(game_dir_name: &str, sessions: &[PlaySession]) {
    let path = history_path(game_dir_name);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    if let Ok(content) = serde_json::to_string_pretty(sessions) {
        tokio::fs::write(&path, content).await.ok();
    }
}

pub async fn record_session(
    game_dir_name: &str,
    start_time: u64,
    end_time: u64,
    account_uuid: &str,
    account_name: &str,
) {
    let mut sessions = load_sessions(game_dir_name).await;
    sessions.push(PlaySession {
        start_time,
        end_time,
        duration_ms: end_time.saturating_sub(start_time),
        account_uuid: account_uuid.to_string(),
        account_name: account_name.to_string(),
    });
    if sessions.len() > MAX_SESSIONS {
        sessions.drain(0..sessions.len() - MAX_SESSIONS);
    }
    save_sessions(game_dir_name, &sessions).await;
}

pub async fn get_stats(game_dir_name: &str) -> Result<PlayHistoryStats, String> {
    let sessions = load_sessions(game_dir_name).await;

    if sessions.is_empty() {
        return Ok(PlayHistoryStats {
            total_time_minutes: 0,
            session_count: 0,
            avg_session_minutes: 0,
            this_week_minutes: 0,
            last_played: None,
            recent_sessions: Vec::new(),
        });
    }

    let total_ms: u64 = sessions.iter().map(|s| s.duration_ms).sum();
    let total_minutes = total_ms / 60_000;
    let session_count = sessions.len() as u32;
    let avg_minutes = if session_count > 0 {
        total_minutes / session_count as u64
    } else {
        0
    };

    let week_ago = current_time_ms().saturating_sub(7 * 24 * 60 * 60 * 1000);
    let week_ms: u64 = sessions
        .iter()
        .filter(|s| s.start_time >= week_ago)
        .map(|s| s.duration_ms)
        .sum();

    Ok(PlayHistoryStats {
        total_time_minutes: total_minutes,
        session_count,
        avg_session_minutes: avg_minutes,
        this_week_minutes: week_ms / 60_000,
        last_played: sessions.iter().map(|s| s.end_time).max(),
        recent_sessions: sessions.iter().rev().take(5).cloned().collect(),
    })
}
