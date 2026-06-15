use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;

use crate::context::use_ctx;
use crate::tauri_bridge;

#[component]
pub fn TopBar() -> impl IntoView {
    let ctx = use_ctx();

    let username = move || ctx.account.get().map(|a| a.username).unwrap_or_default();
    let uuid     = move || ctx.account.get().map(|a| a.uuid).unwrap_or_default();

    let selected_tag = move || {
        let id = ctx.selected_instance.get()?;
        let i  = ctx.instances.get().into_iter().find(|i| i.game_dir_name == id)?;
        Some(format!("{} - {}", i.mc_version, i.version))
    };

    let update_available = RwSignal::new(Option::<String>::None);

    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(Some(info)) = tauri_bridge::invoke0::<Option<serde_json::Value>>("check_launcher_update").await {
                let version = info["version"].as_str().unwrap_or("").to_string();
                update_available.set(Some(version));
            }
        });
    });

    view! {
        <header class="topbar">
            <span class="topbar-logo">"C O R V U S"</span>

            {move || selected_tag().map(|tag| view! {
                <span class="topbar-instance-tag">{tag}</span>
            })}

            <Show when=move || update_available.get().is_some()>
                <div class="update-banner" on:click=move |_| {
                    spawn_local(async { tauri_bridge::invoke0::<()>("download_and_install_update").await.ok(); });
                }>
                    "⬆ Update available "
                    {move || update_available.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="topbar-spacer" />

            <A href="/profile" attr:class="topbar-account">
                <img
                    class="account-avatar"
                    src=move || format!("https://mc-heads.net/avatar/{}/24", uuid())
                    alt="avatar"
                />
                <span>{username}</span>
            </A>
        </header>
    }
}