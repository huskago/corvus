use leptos::{prelude::*, task::spawn_local, web_sys};
use wasm_bindgen::JsCast;

use crate::{
    context::use_ctx,
    models::{AccountInfo, AuthMode, DeviceCodeInfo},
    tauri_bridge as tauri,
};

#[component]
pub fn ProfilePage() -> impl IntoView {
    let ctx = use_ctx();
    let accounts = RwSignal::new(Vec::<AccountInfo>::new());
    let error = RwSignal::new(String::new());
    let adding = RwSignal::new(false);
    let device_info = RwSignal::new(Option::<DeviceCodeInfo>::None);
    let polling = RwSignal::new(false);
    let offline_name = RwSignal::new(String::new());

    let reload = move || {
        spawn_local(async move {
            if let Ok(list) = tauri::invoke0::<Vec<AccountInfo>>("get_accounts").await {
                accounts.set(list);
            }
        });
    };

    Effect::new(move |_| reload());

    let switch_account = move |uuid: String| {
        spawn_local(async move {
            #[derive(serde::Serialize)]
            struct A {
                uuid: String,
            }
            if tauri::invoke::<(), _>("set_active_account", &A { uuid })
                .await
                .is_ok()
            {
                if let Ok(acc) = tauri::invoke0::<Option<AccountInfo>>("get_active_account").await {
                    ctx.account.set(acc);
                }
                reload();
            }
        });
    };

    let remove_account = move |uuid: String| {
        spawn_local(async move {
            #[derive(serde::Serialize)]
            struct A {
                uuid: String,
            }
            if tauri::invoke::<(), _>("remove_account", &A { uuid })
                .await
                .is_ok()
            {
                if let Ok(acc) = tauri::invoke0::<Option<AccountInfo>>("get_active_account").await {
                    ctx.account.set(acc);
                }
                reload();
            }
        });
    };

    let start_ms = move |_| {
        error.set(String::new());
        spawn_local(async move {
            match tauri::invoke0::<DeviceCodeInfo>("start_device_code_flow").await {
                Ok(info) => {
                    let info_clone = info.clone();
                    device_info.set(Some(info));
                    polling.set(true);
                    spawn_local(async move {
                        #[derive(serde::Serialize)]
                        #[serde(rename_all = "camelCase")]
                        struct PollArgs {
                            device_code: String,
                            interval: u32,
                        }
                        let res = tauri::invoke::<crate::models::AuthResult, _>(
                            "poll_device_code",
                            &PollArgs {
                                device_code: info_clone.device_code,
                                interval: info_clone.interval,
                            },
                        )
                        .await;
                        polling.set(false);
                        device_info.set(None);
                        adding.set(false);
                        match res {
                            Ok(r) if r.success => {
                                ctx.account.set(r.account);
                                reload();
                            }
                            Ok(r) => error.set(r.error.unwrap_or("Auth failed".into())),
                            Err(e) => error.set(e),
                        }
                    });
                }
                Err(e) => error.set(e),
            }
        });
    };

    let cancel_ms = move |_| {
        polling.set(false);
        device_info.set(None);
        adding.set(false);
        spawn_local(async {
            tauri::invoke0::<()>("cancel_device_code_flow").await.ok();
        });
    };

    let add_offline = move |_| {
        let name = offline_name.get();
        if name.trim().is_empty() {
            return;
        }
        spawn_local(async move {
            #[derive(serde::Serialize)]
            struct A {
                username: String,
            }
            if let Ok(r) = tauri::invoke::<crate::models::AuthResult, _>(
                "add_offline_account",
                &A { username: name },
            )
            .await
            {
                if r.success {
                    ctx.account.set(r.account);
                    reload();
                    adding.set(false);
                } else {
                    error.set(r.error.unwrap_or_default());
                }
            }
        });
    };

    view! {
        <div class="page-container">
            <div class="page-header">
                <h1 class="page-title">"Profile"</h1>
                <button class="btn-accent"
                    on:click=move |_| { adding.update(|v| *v = !*v); error.set(String::new()); }>
                    {move || if adding.get() { "Cancel" } else { "+ Add Account" }}
                </button>
            </div>

            {move || {
                if polling.get() {
                    view! {
                        <div style="display:flex;align-items:center;gap:12px;padding:8px 0">
                            <div class="spinner" />
                            <span style="color:var(--text-muted)">
                                "Waiting for Microsoft approval…"
                            </span>
                            <button class="btn-ghost-sm" on:click=cancel_ms>"Cancel"</button>
                        </div>
                    }.into_any()
                } else if device_info.get().is_some() {
                    view! {
                        <div class="login-code-box">
                            <div class="login-code">
                                {move || device_info.get().map(|d| d.user_code).unwrap_or_default()}
                            </div>
                            <div class="login-code-hint">
                                "Go to "
                                {move || device_info.get().map(|d| d.verification_uri).unwrap_or_default()}
                                " and enter this code"
                            </div>
                        </div>
                    }.into_any()
                } else {
                    let allow_offline = ctx.build_config.get().allow_offline;
                    view! {
                        <div style="display:flex;flex-direction:column;gap:10px">
                            <button class="btn btn-primary" on:click=start_ms>
                                "🪟 Sign in with Microsoft"
                            </button>
                            <Show when=move || allow_offline>
                                <div class="login-divider">"or"</div>
                                <div style="display:flex;gap:8px">
                                    <input
                                        class="login-input"
                                        type="text"
                                        placeholder="Offline username (max 16)"
                                        maxlength="16"
                                        prop:value=offline_name
                                        on:input=move |e| {
                                            let v = e.target()
                                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                                .map(|i| i.value())
                                                .unwrap_or_default();
                                            offline_name.set(v);
                                        }
                                    />
                                    <button class="btn-accent" on:click=add_offline>"Add"</button>
                                </div>
                            </Show>
                        </div>
                    }.into_any()
                }
            }}

            <Show when=move || !error.get().is_empty()>
                <div class="error-banner">{error}</div>
            </Show>

            <div style="display:flex;flex-direction:column;gap:10px;margin-top:16px">
                <For
                    each=move || accounts.get()
                    key=|a| a.uuid.clone()
                    children=move |account| {
                        let uuid1 = account.uuid.clone();
                        let uuid2 = account.uuid.clone();
                        let is_active = account.is_active;
                        let is_ms = account.auth_mode == AuthMode::Microsoft;
                        view! {
                            <div class=if is_active { "account-card active" } else { "account-card" }>
                                <img
                                    class="account-avatar-lg"
                                    src=format!("https://mc-heads.net/avatar/{}/40", account.uuid)
                                    alt=""
                                />
                                <div class="account-info">
                                    <div class="account-name">{account.username.clone()}</div>
                                    <div class="account-uuid">{account.uuid.clone()}</div>
                                    <span class=if is_ms { "auth-badge ms" } else { "auth-badge offline" }>
                                        {if is_ms { "Microsoft" } else { "Offline" }}
                                    </span>
                                </div>
                                <div class="account-actions">
                                    {if is_active {
                                        view! { <span class="active-badge">"✓ Active"</span> }.into_any()
                                    } else {
                                        view! {
                                            <button class="btn-accent"
                                                on:click=move |_| switch_account(uuid1.clone())>
                                                "Use"
                                            </button>
                                        }.into_any()
                                    }}
                                    <button class="btn-danger"
                                        on:click=move |_| remove_account(uuid2.clone())>
                                        "Remove"
                                    </button>
                                </div>
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}
