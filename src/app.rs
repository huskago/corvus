use leptos::{prelude::*, task::spawn_local, web_sys};

use leptos_router::{components::*, path};

use crate::{context::AppCtx, models::ProgressPayload, tauri_bridge as tauri};

use crate::pages::{
    home::HomePage, instance_settings::InstanceSettingsPage, login::LoginPage,
    profile::ProfilePage, settings::SettingsPage,
};

#[component]
pub fn App() -> impl IntoView {
    let ctx = AppCtx::new();
    provide_context(ctx);

    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(bc) =
                tauri::invoke0::<crate::models::LauncherBuildConfig>("get_launcher_build_config").await
            {
                ctx.build_config.set(bc);
            }
            if let Ok(account) =
                tauri::invoke0::<Option<crate::models::AccountInfo>>("get_active_account").await
            {
                ctx.account.set(account);
            }
            if let Ok(config) =
                tauri::invoke0::<crate::models::LauncherConfig>("get_launcher_config").await
            {
                if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                    if let Some(root) = doc.document_element() {
                        let theme = if config.theme == "LIGHT" {
                            "light"
                        } else {
                            "dark"
                        };
                        let _ = root.set_attribute("data-theme", theme);
                    }
                }
                ctx.config.set(config);
            }
        });
    });

    setup_launch_listeners(ctx);
    setup_polling(ctx);

    view! {
        <Show
            when=move || ctx.account.get().is_some()
            fallback=move || view! { <LoginPage /> }
        >
            <Show when=move || ctx.update_available.get().is_some()>
                <div class="update-banner">
                    {move || ctx.update_available.get().map(|u| format!("Update v{} available", u.version)).unwrap_or_default()}
                    <button class="btn-accent" on:click=move |_| {
                        spawn_local(async move {
                            tauri::invoke0::<()>("download_and_install_update").await.ok();
                        });
                    }>"Install & Restart"</button>
                </div>
            </Show>
            <Router>
                <div id="app">
                    <crate::components::topbar::TopBar />
                    <div class="main-layout">
                        <crate::components::sidebar::Sidebar />
                        <main class="content">
                            <Routes fallback=|| "Page not found">
                                <Route path=path!("/") view=HomePage />
                                <Route path=path!("/profile") view=ProfilePage />
                                <Route path=path!("/settings") view=SettingsPage />
                                <Route path=path!("/instance/:id/settings") view=InstanceSettingsPage />
                            </Routes>
                        </main>
                    </div>
                </div>
            </Router>
        </Show>
    }
}

fn setup_polling(ctx: AppCtx) {
    let _ = set_interval(
        move || {
            spawn_local(async move {
                if let Ok(list) = tauri::invoke0::<Vec<crate::models::InstanceInfo>>("fetch_instances").await {
                    ctx.instances.set(list);
                }
            });
        },
        std::time::Duration::from_secs(300),
    );

    let _ = set_interval(
        move || {
            spawn_local(async move {
                if let Ok(Some(info)) = tauri::invoke0::<Option<crate::models::UpdateInfo>>("check_launcher_update").await {
                    ctx.update_available.set(Some(info));
                }
            });
        },
        std::time::Duration::from_secs(600),
    );
}

fn setup_launch_listeners(ctx: AppCtx) {
    tauri::listen("launch:status", move |e| {
        #[derive(serde::Deserialize)]
        struct P {
            message: String,
        }
        if let Ok(p) = serde_wasm_bindgen::from_value::<P>(e) {
            ctx.launch_status.set(p.message);
        }
    });

    tauri::listen("launch:progress", move |e| {
        if let Ok(p) = serde_wasm_bindgen::from_value::<ProgressPayload>(e) {
            if p.files_total > 0 {
                ctx.launch_progress
                    .set(p.files_done as f32 / p.files_total as f32);
            }
            ctx.launch_files_done.set(p.files_done);
            ctx.launch_files_total.set(p.files_total);
            ctx.launch_bytes_done.set(p.bytes_done);
            ctx.launch_speed_bps.set(p.speed_bps);
        }
    });

    tauri::listen("launch:log", move |e| {
        #[derive(serde::Deserialize)]
        struct P {
            line: String,
        }
        if let Ok(p) = serde_wasm_bindgen::from_value::<P>(e) {
            ctx.push_console_line(p.line);
        }
    });

    tauri::listen("launch:done", move |_| {
        ctx.is_launching.set(false);
        ctx.launch_status.set("Minecraft closed.".into());
        ctx.launch_progress.set(0.0);
        ctx.launch_files_done.set(0);
        ctx.launch_files_total.set(0);
        ctx.launch_bytes_done.set(0);
        ctx.launch_speed_bps.set(0);
    });

    tauri::listen("launch:error", move |e| {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct P {
            message: String,
            game_dir_name: String,
        }
        ctx.is_launching.set(false);
        if let Ok(p) = serde_wasm_bindgen::from_value::<P>(e) {
            ctx.launch_status.set(format!("Error: {}", p.message));
            ctx.crash_game_dir.set(Some(p.game_dir_name));
        }
    });
}
