mod app;
mod screens;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use dioxus::desktop::muda::{
        Menu, MenuItem, PredefinedMenuItem, Submenu,
        accelerator::{Accelerator, Code, Modifiers},
    };
    #[cfg(target_os = "macos")]
    use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;

    let bootstrap = auspex_core::bootstrap::bootstrap_controller_from_env();

    // Embed the stylesheet at launch time so it is guaranteed to be in the
    // HTML head before the first render — the JS-eval injection path is
    // async and can miss the first paint or fail silently in some host modes.
    let main_css = include_str!("../assets/main.css");
    let custom_head = format!("<style id=\"auspex-main-css\">{main_css}</style>");

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
        .with_min_inner_size(dioxus::desktop::LogicalSize::new(700.0, 600.0));
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
                .with_custom_head(custom_head)
                .with_on_window(|window, _| {
                    #[cfg(target_os = "macos")]
                    {
                        use dioxus::desktop::tao::platform::macos::WindowExtMacOS;
                        window.set_titlebar_transparent(false);
                        window.set_fullsize_content_view(false);
                    }
                }),
        )
        .with_context(bootstrap)
        .launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    let bootstrap = auspex_core::bootstrap::bootstrap_controller_for_web();

    dioxus::LaunchBuilder::web()
        .with_context(bootstrap)
        .launch(app::App);
}
