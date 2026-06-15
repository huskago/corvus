use crate::models::{AccountInfo, InstanceInfo, LauncherBuildConfig, LauncherConfig, UpdateInfo};
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct AppCtx {
    pub build_config: RwSignal<LauncherBuildConfig>,
    pub account: RwSignal<Option<AccountInfo>>,
    pub selected_instance: RwSignal<Option<String>>,
    pub is_launching: RwSignal<bool>,
    pub launch_status: RwSignal<String>,
    pub launch_progress: RwSignal<f32>,
    pub launch_files_done: RwSignal<u32>,
    pub launch_files_total: RwSignal<u32>,
    pub launch_bytes_done: RwSignal<u64>,
    pub launch_speed_bps: RwSignal<u64>,
    pub console_lines: RwSignal<Vec<String>>,
    pub config: RwSignal<LauncherConfig>,
    pub instances: RwSignal<Vec<InstanceInfo>>,
    pub update_available: RwSignal<Option<UpdateInfo>>,
}

impl AppCtx {
    pub fn new() -> Self {
        Self {
            build_config: RwSignal::new(LauncherBuildConfig::default()),
            account: RwSignal::new(None),
            selected_instance: RwSignal::new(None),
            is_launching: RwSignal::new(false),
            launch_status: RwSignal::new(String::new()),
            launch_progress: RwSignal::new(0.0),
            launch_files_done: RwSignal::new(0),
            launch_files_total: RwSignal::new(0),
            launch_bytes_done: RwSignal::new(0),
            launch_speed_bps: RwSignal::new(0),
            console_lines: RwSignal::new(Vec::new()),
            config: RwSignal::new(LauncherConfig::default()),
            instances: RwSignal::new(Vec::new()),
            update_available: RwSignal::new(None),
        }
    }

    pub fn push_console_line(self, line: String) {
        self.console_lines.update(|lines| {
            lines.push(line);
            if lines.len() > 500 {
                lines.remove(0);
            }
        });
    }
}

pub fn use_ctx() -> AppCtx {
    use_context::<AppCtx>().expect("AppCtx missing, use provide_context(AppCtx::new())")
}
