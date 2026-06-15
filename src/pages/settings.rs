use leptos::{prelude::*, task::spawn_local, web_sys};

use crate::{context::use_ctx, tauri_bridge as tauri};

#[component]
fn AutostartToggle() -> impl IntoView {
    let enabled = RwSignal::new(false);

    spawn_local(async move {
        if let Ok(v) = tauri::invoke0::<bool>("is_autostart_enabled").await {
            enabled.set(v);
        }
    });

    let on_change = move |v: bool| {
        spawn_local(async move {
            let cmd = if v { "enable_autostart" } else { "disable_autostart" };
            if tauri::invoke0::<()>(cmd).await.is_ok() {
                enabled.set(v);
            }
        });
    };

    view! {
        <ToggleRow
            label="Launch on login"
            sublabel="Start Corvus automatically when you log in"
            value=move || enabled.get()
            on_change=on_change
        />
    }
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    let ctx    = use_ctx();
    let saved  = RwSignal::new(false);
    let error  = RwSignal::new(String::new());

    let config = RwSignal::new(ctx.config.get());

    Effect::new(move |_| { config.set(ctx.config.get()); });

    let save = move || {
        let cfg = config.get();
        spawn_local(async move {
            #[derive(serde::Serialize)]
            struct Args { config: crate::models::LauncherConfig }
            match tauri::invoke::<(), _>("save_launcher_config", &Args { config: cfg.clone() }).await {
                Ok(_) => {
                    ctx.config.set(cfg.clone());
                    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                        if let Some(root) = doc.document_element() {
                            let theme = if cfg.theme == "LIGHT" { "light" } else { "dark" };
                            root.set_attribute("data-theme", theme).ok();
                        }
                    }
                    saved.set(true);
                    gloo_timers_or_sleep();
                }
                Err(e) => error.set(e),
            }
        });
    };

    let save_click = move |_| {
        saved.set(false);
        error.set(String::new());
        save();
    };

    view! {
        <div class="page-container">
            <div class="page-header">
                <h1 class="page-title">"Settings"</h1>
                <button class="btn-accent" on:click=save_click>
                    {move || if saved.get() { "✓ Saved!" } else { "Save" }}
                </button>
            </div>

            <Show when=move || !error.get().is_empty()>
                <div class="error-banner">{error}</div>
            </Show>

            <div class="settings-section">
                <div class="settings-section-title">"Appearance"</div>

                <div class="settings-row">
                    <div>
                        <div class="settings-label">"Theme"</div>
                        <div class="settings-sublabel">"Dark or Light mode"</div>
                    </div>
                    <div style="display:flex;gap:8px">
                        <button
                            class=move || if config.get().theme == "DARK" {
                                "theme-btn active"
                            } else {
                                "theme-btn"
                            }
                            on:click=move |_| config.update(|c| c.theme = "DARK".into())
                        >
                            "🌙 Dark"
                        </button>
                        <button
                            class=move || if config.get().theme == "LIGHT" {
                                "theme-btn active"
                            } else {
                                "theme-btn"
                            }
                            on:click=move |_| config.update(|c| c.theme = "LIGHT".into())
                        >
                            "☀️ Light"
                        </button>
                    </div>
                </div>
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"Launcher"</div>

                <ToggleRow
                    label="Keep launcher open after launch"
                    sublabel="The launcher stays visible while the game is running"
                    value=move || config.get().keep_launcher_open
                    on_change=move |v| config.update(|c| c.keep_launcher_open = v)
                />

                <ToggleRow
                    label="Show console"
                    sublabel="Display Minecraft stdout in the launcher"
                    value=move || config.get().show_console
                    on_change=move |v| config.update(|c| c.show_console = v)
                />

                <ToggleRow
                    label="Discord Rich Presence"
                    sublabel="Show what you're playing in Discord"
                    value=move || config.get().discord_rpc
                    on_change=move |v| config.update(|c| c.discord_rpc = v)
                />
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"Startup"</div>
                <AutostartToggle />
            </div>

            <div class="settings-section">
                <div class="settings-section-title">"About"</div>
                <div class="settings-row">
                    <div>
                        <div class="settings-label">"Launcher version"</div>
                        <div class="settings-sublabel">{format!("v{}", env!("CARGO_PKG_VERSION"))}</div>
                    </div>
                </div>
            </div>
        </div>
    }
}

fn gloo_timers_or_sleep() {}

#[component]
fn ToggleRow(
    label: &'static str,
    sublabel: &'static str,
    value: impl Fn() -> bool + Send + Sync + Clone + 'static,
    on_change: impl Fn(bool) + 'static,
) -> impl IntoView {
    let value_for_class = value.clone();
    view! {
        <div class="settings-row">
            <div>
                <div class="settings-label">{label}</div>
                <div class="settings-sublabel">{sublabel}</div>
            </div>
            <button
                class=move || if value_for_class() { "toggle on" } else { "toggle off" }
                on:click=move |_| on_change(!value())
            />
        </div>
    }
}