mod app;
mod audit_timeline;
mod bootstrap;
mod controller;
mod event_stream;
mod fixtures;
mod instance_registry;
mod omegon_control;
mod remote_session;
mod runtime_types;
mod screens;
mod session_model;
mod state_engine;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use dioxus::desktop::muda::{
        accelerator::{Accelerator, Code, Modifiers},
        Menu, MenuItem, PredefinedMenuItem, Submenu,
    };

    let bootstrap = bootstrap::bootstrap_controller_from_env();

    let menu = Menu::new();
    let app_menu = Submenu::new("Auspex", true);
    app_menu
        .append_items(&[
            &MenuItem::with_id(
                "auspex-open-settings",
                "Settings…",
                true,
                Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)),
            ),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ])
        .expect("settings menu should build");
    let edit_menu = Submenu::new("Edit", true);
    edit_menu
        .append_items(&[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::select_all(None),
        ])
        .expect("edit menu should build");
    menu.append_items(&[&app_menu, &edit_menu])
        .expect("desktop menu should build");

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_menu(menu)
                .with_window(
                    dioxus::desktop::WindowBuilder::new()
                        .with_title("Auspex")
                        .with_resizable(true)
                        .with_inner_size(dioxus::desktop::LogicalSize::new(1440.0, 920.0))
                        .with_min_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 760.0)),
                ),
        )
        .with_context(bootstrap)
        .launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Web mode: Omegon is running in a container / remote host.
    // The app bootstraps by connecting to the state endpoint served
    // alongside this page, or from a URL provided via query string.
    dioxus::LaunchBuilder::web()
        .launch(app::App);
}
