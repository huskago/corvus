use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

use crate::models::{InstanceInfo, NewsItem};

pub struct NewsCache {
    pub items: Vec<NewsItem>,
    pub fetched_at: Option<Instant>,
}

impl NewsCache {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            fetched_at: None,
        }
    }
}

pub struct InstancesCache {
    pub items: Vec<InstanceInfo>,
    pub fetched_at: Option<Instant>,
}

impl InstancesCache {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            fetched_at: None,
        }
    }
}

pub struct AppStateInner {
    pub http_client: reqwest::Client,
    pub news_cache: Mutex<NewsCache>,
    pub instances_cache: Mutex<InstancesCache>,
    pub running_instance: Mutex<Option<String>>,
    pub auth_poll_cancelled: std::sync::atomic::AtomicBool,
}

#[derive(Clone)]
pub struct AppState(pub Arc<AppStateInner>);

impl AppState {
    pub fn new() -> Self {
        Self(Arc::new(AppStateInner {
            http_client: reqwest::Client::new(),
            news_cache: Mutex::new(NewsCache::new()),
            instances_cache: Mutex::new(InstancesCache::new()),
            running_instance: Mutex::new(None),
            auth_poll_cancelled: std::sync::atomic::AtomicBool::new(false),
        }))
    }
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
