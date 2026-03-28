mod app;
mod bootstrap;
mod controller;
mod fixtures;
mod omegon_control;
mod remote_session;
mod session_model;

fn main() {
    let bootstrap = bootstrap::bootstrap_controller_from_env();
    dioxus::LaunchBuilder::desktop()
        .with_context(bootstrap)
        .launch(app::App);
}
