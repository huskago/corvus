mod app;
mod components;
mod context;
mod models;
mod pages;
mod tauri_bridge;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
