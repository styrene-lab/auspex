mod app;
mod audit_timeline;
mod bootstrap;
mod command_transport;
mod controller;
mod event_stream;
mod fixtures;
mod instance_registry;
#[cfg(not(target_arch = "wasm32"))]
mod ipc_client;
mod omegon_control;
mod remote_session;
mod runtime_types;
mod screens;
mod session_event;
mod session_model;
mod state_engine;
mod telemetry;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use dioxus::desktop::muda::{
        Menu, MenuItem, PredefinedMenuItem, Submenu,
        accelerator::{Accelerator, Code, Modifiers},
    };
    #[cfg(target_os = "macos")]
    use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;

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

    let window = dioxus::desktop::WindowBuilder::new()
        .with_title("Auspex")
        .with_resizable(true)
        .with_inner_size(dioxus::desktop::LogicalSize::new(1440.0, 920.0))
        .with_min_inner_size(dioxus::desktop::LogicalSize::new(1100.0, 760.0));
    #[cfg(target_os = "macos")]
    let window = window
        .with_titlebar_transparent(false)
        .with_fullsize_content_view(false)
        .with_title_hidden(false);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_menu(menu)
                .with_window(window)
                .with_as_child_window()
                .with_on_window(|window, _| {
                    #[cfg(target_os = "macos")]
                    {
                        use dioxus::desktop::tao::platform::macos::WindowExtMacOS;
                        window.set_titlebar_transparent(false);
                        window.set_fullsize_content_view(false);
                        let inner = window.inner_size();
                        let outer = window.outer_size();
                        let outer_pos = window.outer_position().ok();
                        let inner_pos = window.inner_position().ok();
                        eprintln!(
                            "[auspex-window] inner={:?} outer={:?} outer_pos={:?} inner_pos={:?} scale_factor={} fullscreen={:?}",
                            inner,
                            outer,
                            outer_pos,
                            inner_pos,
                            window.scale_factor(),
                            window.fullscreen().is_some(),
                        );
                        let ns_window = window.ns_window();
                        let ns_view = window.ns_view();
                        eprintln!("[auspex-window] ns_window={:?} ns_view={:?}", ns_window, ns_view);
                    }
                }),
        )
        .with_context(bootstrap)
        .launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    // Web mode: Omegon is running in a container / remote host.
    // The app bootstraps by connecting to the state endpoint served
    // alongside this page, or from a URL provided via query string.
    dioxus::LaunchBuilder::web().launch(app::App);
}
