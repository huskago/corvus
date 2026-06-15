use leptos::{prelude::*, task::spawn_local};
use leptos_router::components::A;

use crate::{
    context::use_ctx,
    models::{InstanceInfo, ModLoader, NewsItem, NewsType},
    tauri_bridge as tauri,
};

#[component]
pub fn HomePage() -> impl IntoView {
    let ctx = use_ctx();
    let news = RwSignal::new(Vec::<NewsItem>::new());
    let error = RwSignal::new(String::new());

    Effect::new(move |_| {
        spawn_local(async move {
            match tauri::invoke0::<Vec<InstanceInfo>>("fetch_instances").await {
                Ok(list) => {
                    if ctx.selected_instance.get().is_none() {
                        if let Some(first) = list.first() {
                            ctx.selected_instance.set(Some(first.game_dir_name.clone()));
                        }
                    }
                    ctx.instances.set(list);
                }
                Err(e) => error.set(e),
            }
            if let Ok(items) = tauri::invoke0::<Vec<NewsItem>>("fetch_news").await {
                news.set(items);
            }
        });
    });

    let selected_info = move || {
        let id = ctx.selected_instance.get()?;
        ctx.instances.get().into_iter().find(|i| i.game_dir_name == id)
    };

    let launch = move |_| {
        let Some(id) = ctx.selected_instance.get() else { return };
        ctx.is_launching.set(true);
        ctx.launch_status.set("Initializing…".into());
        ctx.launch_progress.set(0.0);
        ctx.launch_files_done.set(0);
        ctx.launch_files_total.set(0);
        ctx.launch_bytes_done.set(0);
        ctx.launch_speed_bps.set(0);
        ctx.console_lines.update(|v| v.clear());
        spawn_local(async move {
            #[derive(serde::Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Args { game_dir_name: String }
            tauri::invoke::<(), _>("launch_instance", &Args { game_dir_name: id })
                .await
                .ok();
        });
    };

    let refresh_news = move |_| {
        spawn_local(async move {
            tauri::invoke0::<()>("invalidate_news_cache").await.ok();
            if let Ok(items) = tauri::invoke0::<Vec<NewsItem>>("fetch_news").await {
                news.set(items);
            }
        });
    };

    view! {
        <div class="home">
            <div class="home-header">
                {move || selected_info().map(|i| view! {
                    <span class="home-tag">{i.mc_version} " · " {i.version}</span>
                    <Show when=move || i.maintenance>
                        <span class="home-tag maintenance">"MAINTENANCE"</span>
                    </Show>
                })}
            </div>

            <div class="home-body">
                <div class="home-left">
                    <div class="instances-section">
                        <div class="section-label">"Instances"</div>
                        <Show
                            when=move || !ctx.instances.get().is_empty()
                            fallback=|| view! { <div class="loading">"Loading…"</div> }
                        >
                            <div class="instances-scroll">
                                <For
                                    each=move || ctx.instances.get()
                                    key=|i| i.game_dir_name.clone()
                                    children=move |instance| {
                                        let id  = instance.game_dir_name.clone();
                                        let id2 = id.clone();
                                        let is_sel = move || {
                                            ctx.selected_instance.get().as_deref() == Some(&id)
                                        };
                                        view! {
                                            <InstanceCard
                                                instance=instance
                                                selected=is_sel
                                                on_select=move || {
                                                    ctx.selected_instance.set(Some(id2.clone()))
                                                }
                                            />
                                        }
                                    }
                                />
                            </div>
                        </Show>
                        <Show when=move || !error.get().is_empty()>
                            <p style="color:var(--danger);font-size:12px;margin-top:8px">
                                {error}
                            </p>
                        </Show>
                    </div>

                    <div class="play-section">
                        <Show
                            when=move || selected_info().is_some()
                            fallback=|| view! {
                                <div class="play-section-empty">
                                    "← Select an instance to play"
                                </div>
                            }
                        >
                            {move || selected_info().map(|instance| {
                                let id = instance.game_dir_name.clone();
                                view! {
                                    <div class="play-instance-row">
                                        <span class="play-instance-name">
                                            {instance.name.clone()}
                                        </span>
                                        <A
                                            href=format!("/instance/{}/settings", &id)
                                            attr:class="btn-settings-link"
                                        >
                                            "⚙ Settings"
                                        </A>
                                    </div>

                                    <Show when=move || {
                                        let s = ctx.launch_status.get();
                                        !s.is_empty()
                                        && !s.starts_with("Error:")
                                        && !s.contains("unavailable")
                                        && !s.contains("failed")
                                    }>
                                        <div class="play-status-text">
                                            {move || ctx.launch_status.get()}
                                        </div>
                                    </Show>

                                    <Show when=move || {
                                        let s = ctx.launch_status.get();
                                        s.starts_with("Error:")
                                        || s.contains("unavailable")
                                        || s.contains("failed")
                                    }>
                                        <div class="play-error-text">
                                            "⚠ " {move || ctx.launch_status.get()}
                                        </div>
                                    </Show>

                                    <Show when=move || ctx.is_launching.get()>
                                        <div class="play-progress-track">
                                            <div
                                                class="play-progress-fill"
                                                style=move || format!(
                                                    "width:{}%",
                                                    ctx.launch_progress.get() * 100.0
                                                )
                                            />
                                        </div>
                                        <div class="play-progress-stats">
                                            {move || {
                                                let files_done = ctx.launch_files_done.get();
                                                let files_total = ctx.launch_files_total.get();
                                                let bytes = ctx.launch_bytes_done.get();
                                                let speed = ctx.launch_speed_bps.get();
                                                if files_total == 0 { return String::new(); }
                                                format!(
                                                    "{}/{} files · {} · {}",
                                                    files_done,
                                                    files_total,
                                                    fmt_bytes(bytes),
                                                    fmt_speed(speed),
                                                )
                                            }}
                                        </div>
                                    </Show>

                                    <button
                                        class="btn-play"
                                        disabled=move || ctx.is_launching.get() || instance.maintenance
                                        on:click=launch
                                    >
                                        {move || if ctx.is_launching.get() { "LAUNCHING…" } else { "PLAY" }}
                                    </button>
                                }
                            })}
                        </Show>
                    </div>
                </div>

                <div class="home-right">
                    <div class="news-panel">
                        <div class="news-panel-header">
                            <span>"📰"</span>
                            <span class="news-panel-title">"NEWS"</span>
                            <button class="news-refresh" on:click=refresh_news title="Refresh">
                                "↻"
                            </button>
                        </div>
                        <div class="news-list">
                            <Show
                                when=move || !news.get().is_empty()
                                fallback=|| view! {
                                    <div class="loading">"Loading…"</div>
                                }
                            >
                                <For
                                    each=move || news.get()
                                    key=|n| n.id.clone()
                                    children=|item| view! { <NewsCard item=item /> }
                                />
                            </Show>
                        </div>
                    </div>
                </div>
            </div>

            <Show when=move || {
                ctx.config.get().show_console && !ctx.console_lines.get().is_empty()
            }>
                <div class="console-drawer">
                    <For
                        each=move || ctx.console_lines.get()
                        key=|l| l.clone()
                        children=|line| view! { <div>{line}</div> }
                    />
                </div>
            </Show>
        </div>
    }
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn fmt_speed(bps: u64) -> String {
    if bps == 0 {
        return "— KB/s".to_string();
    }
    if bps >= 1_048_576 {
        format!("{:.1} MB/s", bps as f64 / 1_048_576.0)
    } else {
        format!("{:.0} KB/s", bps as f64 / 1_024.0)
    }
}

#[component]
fn InstanceCard(
    instance: InstanceInfo,
    selected: impl Fn() -> bool + Send + Sync + Clone + 'static,
    on_select: impl Fn() + 'static,
) -> impl IntoView {
    let bg          = instance.bg_url.clone().unwrap_or_default();
    let maintenance = instance.maintenance;

    let selected_for_show = selected.clone();

    let card_class = move || {
        let mut c = String::from("instance-card");
        if selected()   { c.push_str(" selected"); }
        if maintenance  { c.push_str(" maintenance"); }
        c
    };

    let badge = match instance.loader {
        ModLoader::Fabric   => view! { <span class="badge badge-fabric">"Fabric"</span>   }.into_any(),
        ModLoader::Quilt    => view! { <span class="badge badge-quilt">"Quilt"</span>     }.into_any(),
        ModLoader::Forge    => view! { <span class="badge badge-forge">"Forge"</span>     }.into_any(),
        ModLoader::NeoForge => view! { <span class="badge badge-neo">"NeoForge"</span>   }.into_any(),
        ModLoader::Vanilla  => view! { <span class="badge badge-vanilla">"Vanilla"</span> }.into_any(),
    };

    view! {
        <div class=card_class on:click=move |_| on_select()>
            <div
                class="card-bg"
                style=move || if bg.is_empty() {
                    "background-color:#1a1a1a".to_string()
                } else {
                    format!("background-image:url('{bg}')")
                }
            />
            <div class="card-gradient" />

            <Show when=selected_for_show>
                <div class="card-active-dot" />
            </Show>

            <img class="card-icon" src=instance.icon_url alt="" />

            <div class="card-footer">
                <div class="card-name">{instance.name}</div>
                <div class="card-meta">
                    <span>{instance.mc_version}</span>
                    {badge}
                </div>
            </div>
        </div>
    }
}

#[component]
fn NewsCard(item: NewsItem) -> impl IntoView {
    let (badge_class, badge_label) = match item.news_type {
        NewsType::Update      => ("news-badge badge-update",      "Update"),
        NewsType::Event       => ("news-badge badge-event",       "Event"),
        NewsType::Maintenance => ("news-badge badge-maintenance", "Maintenance"),
        NewsType::Info        => ("news-badge badge-info",        "Info"),
    };
    let date_str = item.date.get(..10).unwrap_or(&item.date).to_string();

    view! {
        <div class="news-item">
            <div class="news-item-title">{item.title}</div>
            <div class="news-item-meta">
                <span class=badge_class>{badge_label}</span>
                <span>{date_str}</span>
            </div>
        </div>
    }
}