use leptos::{prelude::*, task::spawn_local, web_sys};
use leptos_router::hooks::use_params_map;
use wasm_bindgen::JsCast;

use crate::{
    context::use_ctx,
    models::{BackupInfo, InstanceManifest, LocalInstanceConfig, ModStatus, PlayHistoryStats},
    tauri_bridge as tauri,
};

#[component]
pub fn InstanceSettingsPage() -> impl IntoView {
    let ctx = use_ctx();
    let params = use_params_map();

    let game_dir_name = move || params.with(|p| p.get("id").unwrap_or_default());

    let active_tab = RwSignal::new("java");
    let config = RwSignal::new(LocalInstanceConfig::default());
    let manifest = RwSignal::new(Option::<InstanceManifest>::None);
    let backups = RwSignal::new(Vec::<BackupInfo>::new());
    let history = RwSignal::new(Option::<PlayHistoryStats>::None);
    let error = RwSignal::new(String::new());
    let saved = RwSignal::new(false);
    let busy = RwSignal::new(false);

    let instance_info = move || {
        ctx.instances
            .get()
            .into_iter()
            .find(|i| i.game_dir_name == game_dir_name())
    };

    Effect::new(move |_| {
        let id = game_dir_name();
        if id.is_empty() {
            return;
        }
        spawn_local(async move {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Id {
                game_dir_name: String,
            }
            if let Ok(cfg) = tauri::invoke::<LocalInstanceConfig, _>(
                "get_instance_local_config",
                &Id {
                    game_dir_name: id.clone(),
                },
            )
            .await
            {
                config.set(cfg);
            }

            if let Some(inst) = ctx
                .instances
                .get()
                .into_iter()
                .find(|i| i.game_dir_name == id)
            {
                #[derive(serde::Serialize)]
                #[serde(rename_all = "camelCase")]
                struct U {
                    update_url: String,
                }
                if let Ok(m) = tauri::invoke::<InstanceManifest, _>(
                    "fetch_instance_manifest",
                    &U {
                        update_url: inst.update_url,
                    },
                )
                .await
                {
                    manifest.set(Some(m));
                }
            }

            if let Ok(list) = tauri::invoke::<Vec<BackupInfo>, _>(
                "list_backups",
                &Id {
                    game_dir_name: id.clone(),
                },
            )
            .await
            {
                backups.set(list);
            }

            if let Ok(h) = tauri::invoke::<PlayHistoryStats, _>(
                "get_play_history",
                &Id {
                    game_dir_name: id.clone(),
                },
            )
            .await
            {
                history.set(Some(h));
            }
        });
    });

    let save = move |_| {
        let id = game_dir_name();
        let cfg = config.get();
        error.set(String::new());
        spawn_local(async move {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Args {
                game_dir_name: String,
                config: LocalInstanceConfig,
            }
            match tauri::invoke::<(), _>(
                "save_instance_local_config",
                &Args {
                    game_dir_name: id,
                    config: cfg,
                },
            )
            .await
            {
                Ok(_) => saved.set(true),
                Err(e) => error.set(e),
            }
        });
    };

    let instance_name = move || {
        instance_info()
            .map(|i| i.name)
            .unwrap_or_else(|| game_dir_name())
    };

    view! {
        <div class="page-container">
            <div class="page-header">
                <div>
                    <div style="font-size:12px;color:var(--text-muted);margin-bottom:4px">
                        "Instance Settings"
                    </div>
                    <h1 class="page-title">{instance_name}</h1>
                </div>
                <button class="btn-accent" on:click=save>
                    {move || if saved.get() { "✓ Saved!" } else { "Save" }}
                </button>
            </div>

            <Show when=move || !error.get().is_empty()>
                <div class="error-banner">{error}</div>
            </Show>

            <div class="tabs">
                {["java", "mods", "display", "server", "backups"].map(|tab| view! {
                    <button
                        class=move || if active_tab.get() == tab { "tab-btn active" } else { "tab-btn" }
                        on:click=move |_| { active_tab.set(tab); saved.set(false); }
                    >
                        {tab.to_uppercase()}
                    </button>
                })}
            </div>

            <div class="tab-content">
                <Show when=move || active_tab.get() == "java">
                    <JavaTab config=config />
                </Show>
                <Show when=move || active_tab.get() == "mods">
                    <ModsTab manifest=manifest config=config />
                </Show>
                <Show when=move || active_tab.get() == "display">
                    <DisplayTab config=config />
                </Show>
                <Show when=move || active_tab.get() == "server">
                    <ServerTab config=config instance_info=instance_info />
                </Show>
                <Show when=move || active_tab.get() == "backups">
                    <BackupsTab
                        game_dir_name=game_dir_name
                        backups=backups
                        history=history
                        busy=busy
                    />
                </Show>
            </div>
        </div>
    }
}

#[component]
fn JavaTab(config: RwSignal<LocalInstanceConfig>) -> impl IntoView {
    view! {
        <div class="settings-section">
            <div class="settings-section-title">"Memory"</div>

            <div class="settings-row">
                <div>
                    <div class="settings-label">"Min RAM (MB)"</div>
                    <div class="settings-sublabel">"Minimum heap size (Xms)"</div>
                </div>
                <input
                    class="num-input"
                    type="number" min="512" max="32768" step="256"
                    prop:value=move || config.get().min_ram
                    on:change=move |e| {
                        let v = e.target()
                            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                            .and_then(|i| i.value().parse::<u32>().ok())
                            .unwrap_or(1024);
                        config.update(|c| c.min_ram = v);
                    }
                />
            </div>

            <div class="settings-row">
                <div>
                    <div class="settings-label">"Max RAM (MB)"</div>
                    <div class="settings-sublabel">"Maximum heap size (Xmx)"</div>
                </div>
                <input
                    class="num-input"
                    type="number" min="512" max="32768" step="256"
                    prop:value=move || config.get().max_ram
                    on:change=move |e| {
                        let v = e.target()
                            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                            .and_then(|i| i.value().parse::<u32>().ok())
                            .unwrap_or(4096);
                        config.update(|c| c.max_ram = v);
                    }
                />
            </div>
        </div>

        <div class="settings-section">
            <div class="settings-section-title">"Java"</div>

            <div class="settings-row">
                <div>
                    <div class="settings-label">"Java path"</div>
                    <div class="settings-sublabel">"Leave empty to auto-detect"</div>
                </div>
                <input
                    class="text-input"
                    type="text"
                    placeholder="e.g. C:/Program Files/Java/jdk-21/bin/java.exe"
                    prop:value=move || config.get().java_path
                    on:input=move |e| {
                        let v = e.target()
                            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                            .map(|i| i.value()).unwrap_or_default();
                        config.update(|c| c.java_path = v);
                    }
                />
            </div>

            <div class="settings-row">
                <div>
                    <div class="settings-label">"Extra JVM arguments"</div>
                    <div class="settings-sublabel">"Space-separated flags"</div>
                </div>
                <input
                    class="text-input"
                    type="text"
                    placeholder="-XX:+UseZGC"
                    prop:value=move || config.get().jvm_args
                    on:input=move |e| {
                        let v = e.target()
                            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                            .map(|i| i.value()).unwrap_or_default();
                        config.update(|c| c.jvm_args = v);
                    }
                />
            </div>

            <div class="settings-row">
                <div>
                    <div class="settings-label">"Optimized GC flags"</div>
                    <div class="settings-sublabel">"Enable G1GC optimizations for better performance"</div>
                </div>
                <button
                    class=move || if config.get().optimized_args { "toggle on" } else { "toggle off" }
                    on:click=move |_| config.update(|c| c.optimized_args = !c.optimized_args)
                />
            </div>
        </div>
    }
}

#[component]
fn ModsTab(
    manifest: RwSignal<Option<InstanceManifest>>,
    config: RwSignal<LocalInstanceConfig>,
) -> impl IntoView {
    view! {
        {move || manifest.get().map(|m| {
            let mods          = m.mods;
            let resource_packs = m.resource_packs;
            let shaders       = m.shaders;

            let mods_empty = mods.is_empty();
            let rp_empty   = resource_packs.is_empty();
            let sh_empty   = shaders.is_empty();

            view! {
                {(!mods_empty).then(|| view! {
                    <div class="settings-section">
                        <div class="settings-section-title">"Mods"</div>
                        {mods.into_iter().map(|f| {
                            let is_req = f.status == ModStatus::Required;
                            let badge_class = match &f.status {
                                ModStatus::Required    => "mod-badge required",
                                ModStatus::OptionalOn  => "mod-badge optional-on",
                                ModStatus::OptionalOff => "mod-badge optional-off",
                            };
                            let badge_label = match &f.status {
                                ModStatus::Required    => "Required",
                                ModStatus::OptionalOn  => "Optional (on)",
                                ModStatus::OptionalOff => "Optional (off)",
                            };
                            let display_name = f.name.clone();

                            let n_on  = f.name.clone();
                            let n_tog = f.name.clone();
                            let s_on  = f.status.clone();
                            let s_tog = f.status.clone();

                            let is_on = move || {
                                let cfg = config.get();
                                match s_on {
                                    ModStatus::OptionalOn  => !cfg.disabled_mods.contains(&n_on),
                                    ModStatus::OptionalOff =>  cfg.enabled_mods.contains(&n_on),
                                    _ => true,
                                }
                            };

                            let n_tog2 = n_tog.clone();
                            let toggle = move |_| {
                                let currently_on = {
                                    let cfg = config.get();
                                    match s_tog {
                                        ModStatus::OptionalOn  => !cfg.disabled_mods.contains(&n_tog),
                                        ModStatus::OptionalOff =>  cfg.enabled_mods.contains(&n_tog),
                                        _ => true,
                                    }
                                };
                                config.update(|c| {
                                    if currently_on {
                                        c.enabled_mods.retain(|n| n != &n_tog2);
                                        if !c.disabled_mods.contains(&n_tog2) {
                                            c.disabled_mods.push(n_tog2.clone());
                                        }
                                    } else {
                                        c.disabled_mods.retain(|n| n != &n_tog2);
                                        if !c.enabled_mods.contains(&n_tog2) {
                                            c.enabled_mods.push(n_tog2.clone());
                                        }
                                    }
                                });
                            };

                            view! {
                                <div class="mod-row">
                                    <div class="mod-info">
                                        <span class="mod-name">{display_name}</span>
                                        <span class=badge_class>{badge_label}</span>
                                    </div>
                                    {if is_req {
                                        view! {
                                            <span class="toggle-disabled">"Locked"</span>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <button
                                                class=move || {
                                                    if is_on() { "toggle on" } else { "toggle off" }
                                                }
                                                on:click=toggle
                                            />
                                        }.into_any()
                                    }}
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                })}

                {(!rp_empty).then(|| view! {
                    <div class="settings-section">
                        <div class="settings-section-title">"Resource Packs"</div>
                        {resource_packs.into_iter().map(|f| view! {
                            <div class="mod-row">
                                <span class="mod-name">{f.name}</span>
                            </div>
                        }).collect::<Vec<_>>()}
                    </div>
                })}

                {(!sh_empty).then(|| view! {
                    <div class="settings-section">
                        <div class="settings-section-title">"Shaders"</div>
                        {shaders.into_iter().map(|f| view! {
                            <div class="mod-row">
                                <span class="mod-name">{f.name}</span>
                            </div>
                        }).collect::<Vec<_>>()}
                    </div>
                })}
            }
        })}
    }
}

#[component]
fn DisplayTab(config: RwSignal<LocalInstanceConfig>) -> impl IntoView {
    view! {
        <div class="settings-section">
            <div class="settings-section-title">"Resolution"</div>
            <div class="settings-sublabel" style="margin-bottom:16px">
                "Set to 0 × 0 to use the default window size"
            </div>
            <div style="display:flex;gap:12px;align-items:center">
                <div style="flex:1">
                    <div class="settings-label">"Width"</div>
                    <input
                        class="num-input" style="width:100%;margin-top:6px"
                        type="number" min="0" max="7680"
                        prop:value=move || config.get().resolution_width
                        on:change=move |e| {
                            let v = e.target()
                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                .and_then(|i| i.value().parse::<u32>().ok()).unwrap_or(0);
                            config.update(|c| c.resolution_width = v);
                        }
                    />
                </div>
                <span style="color:var(--text-muted);margin-top:20px">"×"</span>
                <div style="flex:1">
                    <div class="settings-label">"Height"</div>
                    <input
                        class="num-input" style="width:100%;margin-top:6px"
                        type="number" min="0" max="4320"
                        prop:value=move || config.get().resolution_height
                        on:change=move |e| {
                            let v = e.target()
                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                .and_then(|i| i.value().parse::<u32>().ok()).unwrap_or(0);
                            config.update(|c| c.resolution_height = v);
                        }
                    />
                </div>
            </div>
        </div>
    }
}

#[component]
fn ServerTab(
    config: RwSignal<LocalInstanceConfig>,
    instance_info: impl Fn() -> Option<crate::models::InstanceInfo> + Send + 'static,
) -> impl IntoView {
    view! {
        <div class="settings-section">
            <div class="settings-section-title">"Auto-connect"</div>

            <div class="settings-row">
                <div>
                    <div class="settings-label">"Auto-connect to server on launch"</div>
                    <div class="settings-sublabel">
                        {move || instance_info()
                            .and_then(|i| i.server_ip)
                            .map(|ip| format!("Server: {ip}"))
                            .unwrap_or("No server configured for this instance".into())
                        }
                    </div>
                </div>
                <button
                    class=move || if config.get().auto_connect_server { "toggle on" } else { "toggle off" }
                    on:click=move |_| config.update(|c| c.auto_connect_server = !c.auto_connect_server)
                />
            </div>
        </div>
    }
}

#[component]
fn BackupsTab(
    game_dir_name: impl Fn() -> String + 'static + Copy + Send + Sync,
    backups: RwSignal<Vec<BackupInfo>>,
    history: RwSignal<Option<PlayHistoryStats>>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    let reload_backups = move || {
        let id = game_dir_name();
        spawn_local(async move {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                game_dir_name: String,
            }
            if let Ok(list) =
                tauri::invoke::<Vec<BackupInfo>, _>("list_backups", &A { game_dir_name: id }).await
            {
                backups.set(list);
            }
        });
    };

    let create_backup = move |_| {
        if busy.get() {
            return;
        }
        busy.set(true);
        let id = game_dir_name();
        spawn_local(async move {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                game_dir_name: String,
            }
            tauri::invoke::<serde_json::Value, _>("create_backup", &A { game_dir_name: id })
                .await
                .ok();
            busy.set(false);
            reload_backups();
        });
    };

    let restore = move |filename: String| {
        let id = game_dir_name();
        spawn_local(async move {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct A {
                game_dir_name: String,
                filename: String,
            }
            tauri::invoke::<bool, _>(
                "restore_backup",
                &A {
                    game_dir_name: id,
                    filename,
                },
            )
            .await
            .ok();
        });
    };

    let delete = move |filename: String| {
        let id = game_dir_name();
        spawn_local(async move {
            #[derive(serde::Serialize)]
            struct A {
                game_dir_name: String,
                filename: String,
            }
            if tauri::invoke::<bool, _>(
                "delete_backup",
                &A {
                    game_dir_name: id,
                    filename,
                },
            )
            .await
            .is_ok()
            {
                reload_backups();
            }
        });
    };

    view! {
        <Show when=move || history.get().is_some()>
            <div class="settings-section">
                <div class="settings-section-title">"Play History"</div>
                {move || history.get().map(|h| view! {
                    <div class="stats-grid">
                        <StatBox label="Total playtime"
                            value=format_duration(h.total_time_minutes) />
                        <StatBox label="Sessions"
                            value=h.session_count.to_string() />
                        <StatBox label="Avg session"
                            value=format_duration(h.avg_session_minutes) />
                        <StatBox label="This week"
                            value=format_duration(h.this_week_minutes) />
                    </div>
                })}
            </div>
        </Show>

        <div class="settings-section">
            <div style="display:flex;align-items:center;gap:12px;margin-bottom:14px">
                <div class="settings-section-title" style="margin-bottom:0">"Backups"</div>
                <button class="btn-accent" on:click=create_backup disabled=move || busy.get()>
                    {move || if busy.get() { "Creating…" } else { "+ Create Backup" }}
                </button>
            </div>

            <Show
                when=move || !backups.get().is_empty()
                fallback=|| view! { <div class="settings-sublabel">"No backups yet."</div> }
            >
                <div style="display:flex;flex-direction:column;gap:8px">
                    <For
                        each=move || backups.get()
                        key=|b| b.filename.clone()
                        children=move |backup| {
                            let fn1 = backup.filename.clone();
                            let fn2 = backup.filename.clone();
                            let size_mb = backup.size_bytes / 1_048_576;
                            view! {
                                <div class="backup-row">
                                    <div>
                                        <div class="mod-name">{backup.filename.clone()}</div>
                                        <div class="settings-sublabel">
                                            {format!("{size_mb} MB · {}", backup.created_at)}
                                        </div>
                                    </div>
                                    <div style="display:flex;gap:8px">
                                        <button class="btn-ghost-sm"
                                            on:click=move |_| restore(fn1.clone())>
                                            "Restore"
                                        </button>
                                        <button class="btn-danger-sm"
                                            on:click=move |_| delete(fn2.clone())>
                                            "Delete"
                                        </button>
                                    </div>
                                </div>
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn StatBox(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="stat-box">
            <div class="stat-value">{value}</div>
            <div class="stat-label">{label}</div>
        </div>
    }
}

fn format_duration(minutes: u64) -> String {
    if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h {}m", minutes / 60, minutes % 60)
    }
}
