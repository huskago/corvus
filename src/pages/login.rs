use leptos::{prelude::*, task::spawn_local, web_sys};
use wasm_bindgen::JsCast;

use crate::{
    context::use_ctx,
    models::DeviceCodeInfo,
    tauri_bridge as tauri,
};

#[derive(Clone, PartialEq)]
enum Step {
    Choose,
    DeviceCode(DeviceCodeInfo),
    OfflineInput,
}

#[component]
pub fn LoginPage() -> impl IntoView {
    let ctx        = use_ctx();
    let step       = RwSignal::new(Step::Choose);
    let offline_name = RwSignal::new(String::new());
    let error      = RwSignal::new(String::new());

    let start_ms = move |_| {
        error.set(String::new());
        spawn_local(async move {
            match tauri::invoke0::<DeviceCodeInfo>("start_device_code_flow").await {
                Ok(info) => {
                    let info_clone = info.clone();
                    step.set(Step::DeviceCode(info.clone()));
                    spawn_local(async move {
                        #[derive(serde::Serialize)]
                        #[serde(rename_all = "camelCase")]
                        struct PollArgs { device_code: String, interval: u32 }
                        let res = tauri::invoke::<crate::models::AuthResult, _>(
                            "poll_device_code",
                            &PollArgs {
                                device_code: info_clone.device_code,
                                interval:    info_clone.interval,
                            },
                        ).await;
                        match res {
                            Ok(r) if r.success => { ctx.account.set(r.account); }
                            Ok(r) => {
                                error.set(r.error.unwrap_or("Unknown error".into()));
                                step.set(Step::Choose);
                            }
                            Err(e) => { error.set(e); step.set(Step::Choose); }
                        }
                    });
                }
                Err(e) => error.set(e),
            }
        });
    };

    let cancel = move |_| {
        step.set(Step::Choose);
        spawn_local(async { tauri::invoke0::<()>("cancel_device_code_flow").await.ok(); });
    };

    let open_url = move |url: String| {
        spawn_local(async move {
            #[derive(serde::Serialize)]
            struct A { url: String }
            tauri::invoke::<(), _>("plugin:opener|open_url", &A { url }).await.ok();
        });
    };

    let add_offline = move |_| {
        let name = offline_name.get();
        if name.trim().is_empty() {
            error.set("Please enter a username.".into());
            return;
        }
        error.set(String::new());
        spawn_local(async move {
            #[derive(serde::Serialize)]
            struct A { username: String }
            match tauri::invoke::<crate::models::AuthResult, _>(
                "add_offline_account", &A { username: name }
            ).await {
                Ok(r) if r.success => ctx.account.set(r.account),
                Ok(r) => error.set(r.error.unwrap_or("Error".into())),
                Err(e) => error.set(e),
            }
        });
    };

    view! {
        <div class="login-page">
            <div class="login-glow" />
            <div class="login-card">

                <div class="login-icon-wrap">"⬡"</div>

                <div style="text-align:center">
                    <div class="login-title">"Corvus"</div>
                    <div class="login-subtitle">"Sign in to play"</div>
                </div>

                {move || match step.get() {

                    Step::Choose => {
                        let allow_offline = ctx.build_config.get().allow_offline;
                        view! {
                            <div style="width:100%;display:flex;flex-direction:column;gap:10px">
                                <button class="btn btn-primary" on:click=start_ms>
                                    "🪟  Sign in with Microsoft"
                                </button>
                                <Show when=move || allow_offline>
                                    <div class="login-divider">"or"</div>
                                    <button class="btn btn-ghost"
                                        on:click=move |_| {
                                            step.set(Step::OfflineInput);
                                            error.set(String::new());
                                        }>
                                        "👤  Play offline (cracked)"
                                    </button>
                                </Show>
                            </div>
                        }.into_any()
                    },

                    Step::DeviceCode(info) => {
                        let url  = info.verification_uri.clone();
                        let url2 = url.clone();
                        let code = info.user_code.clone();
                        view! {
                            <div style="width:100%;display:flex;flex-direction:column;gap:14px">
                                <div class="login-code-box">
                                    <div class="login-code">{code}</div>
                                    <div class="login-code-hint">
                                        "Go to the page below and enter this code"
                                    </div>
                                </div>
                                <div class="login-verify-url">{url2.clone()}</div>
                                <button class="btn btn-primary"
                                    on:click=move |_| open_url(url.clone())>
                                    "Open browser"
                                </button>
                                <div style="display:flex;align-items:center;gap:10px">
                                    <div class="spinner" />
                                    <span style="font-size:12px;color:var(--text-muted)">
                                        "Waiting for approval…"
                                    </span>
                                </div>
                                <button class="btn btn-ghost" on:click=cancel>
                                    "Cancel"
                                </button>
                            </div>
                        }.into_any()
                    },

                    Step::OfflineInput => view! {
                        <div style="width:100%;display:flex;flex-direction:column;gap:10px">
                            <input
                                class="login-input"
                                type="text"
                                placeholder="Your username (max 16 characters)"
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
                            <button class="btn btn-primary" on:click=add_offline>
                                "Play"
                            </button>
                            <button class="btn btn-ghost"
                                on:click=move |_| {
                                    step.set(Step::Choose);
                                    error.set(String::new());
                                }>
                                "← Back"
                            </button>
                        </div>
                    }.into_any(),
                }}

                <Show when=move || !error.get().is_empty()>
                    <div class="login-error">{error}</div>
                </Show>
            </div>
        </div>
    }
}