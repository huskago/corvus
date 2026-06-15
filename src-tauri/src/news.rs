use std::time::{Duration, Instant};

use crate::build_config;
use crate::config::base_dir;
use crate::models::NewsItem;
use crate::state::AppState;

const CACHE_DURATION: Duration = Duration::from_secs(5 * 60);

fn cache_file_path() -> std::path::PathBuf {
    base_dir().join("cache").join("news.json")
}

pub async fn get_news(state: &AppState) -> Result<Vec<NewsItem>, String> {
    {
        let cache = state.news_cache.lock().await;
        let is_fresh = cache
            .fetched_at
            .map(|t| t.elapsed() < CACHE_DURATION)
            .unwrap_or(false);

        if is_fresh {
            return Ok(cache.items.clone());
        }
    }

    match fetch_from_network(&state.http_client).await {
        Ok(items) => {
            let mut cache = state.news_cache.lock().await;
            cache.items = items.clone();
            cache.fetched_at = Some(Instant::now());
            drop(cache);

            let _ = save_to_disk(&items).await;

            Ok(items)
        }

        Err(network_err) => match load_from_disk().await {
            Ok(items) if !items.is_empty() => {
                eprintln!("[news] Network unavailable, disk cache in use: {network_err}");
                Ok(items)
            }
            _ => Err(format!("Unable to load news: {network_err}")),
        },
    }
}

pub async fn invalidate(state: &AppState) {
    let mut cache = state.news_cache.lock().await;
    cache.fetched_at = None;
}

async fn fetch_from_network(client: &reqwest::Client) -> Result<Vec<NewsItem>, String> {
    let response = client
        .get(&build_config::get().server.news_url)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("The server responded with {}", response.status()));
    }

    response
        .json::<Vec<NewsItem>>()
        .await
        .map_err(|e| format!("Invalid JSON: {e}"))
}

async fn save_to_disk(items: &[NewsItem]) -> Result<(), String> {
    let path = cache_file_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let content = serde_json::to_string_pretty(items).map_err(|e| format!("Serialization: {e}"))?;
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("Write to disk: {e}"))
}

async fn load_from_disk() -> Result<Vec<NewsItem>, String> {
    let path = cache_file_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Disk read: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Invalid JSON: {e}"))
}
