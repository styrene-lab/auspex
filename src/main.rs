mod app;
mod audit_timeline;
mod bootstrap;
mod controller;
mod event_stream;
mod fixtures;
mod omegon_control;
mod remote_session;
mod runtime_types;
mod screens;
mod session_model;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let bootstrap = bootstrap::bootstrap_controller_from_env();
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new().with_window(
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
