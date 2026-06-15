use leptos::prelude::*;
use leptos_router::{components::A, hooks::use_location};

use crate::context::use_ctx;

#[component]
pub fn Sidebar() -> impl IntoView {
    let ctx      = use_ctx();
    let location = use_location();

    let is         = move |path: &'static str| move || location.pathname.get() == path;
    let uuid       = move || ctx.account.get().map(|a| a.uuid).unwrap_or_default();
    let on_profile = move || location.pathname.get() == "/profile";

    view! {
        <nav class="sidebar">
            <div class="sidebar-logo">
                <svg width="22" height="22" viewBox="0 0 24 24" fill="none"
                     stroke="currentColor" stroke-width="2">
                    <polygon points="12 2 22 8.5 22 15.5 12 22 2 15.5 2 8.5"/>
                </svg>
            </div>

            <NavBtn href="/" active=is("/") label="Home">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none"
                     stroke="currentColor" stroke-width="2">
                    <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>
                    <polyline points="9 22 9 12 15 12 15 22"/>
                </svg>
            </NavBtn>

            <NavBtn href="/settings" active=is("/settings") label="Settings">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none"
                     stroke="currentColor" stroke-width="2">
                    <circle cx="12" cy="12" r="3"/>
                    <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83
                             2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33
                             1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09
                             A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33
                             l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06
                             A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3
                             a2 2 0 010-4h.09A1.65 1.65 0 004.6 9
                             a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83
                             2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68
                             a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09
                             a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33
                             l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06
                             A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21
                             a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/>
                </svg>
            </NavBtn>

            <div class="sidebar-spacer" />

            <A
                href="/profile"
                attr:class=move || if on_profile() {
                    "sidebar-avatar-btn active"
                } else {
                    "sidebar-avatar-btn"
                }
                attr:title="My profile"
            >
                <img
                    src=move || format!("https://mc-heads.net/avatar/{}/40", uuid())
                    alt="profile"
                />
            </A>
        </nav>
    }
}

#[component]
fn NavBtn(
    href: &'static str,
    active: impl Fn() -> bool + Send + Sync + 'static,
    label: &'static str,
    children: Children,
) -> impl IntoView {
    view! {
        <A
            href=href
            attr:class=move || if active() { "nav-icon-btn active" } else { "nav-icon-btn" }
            attr:title=label
        >
            {children()}
        </A>
    }
}