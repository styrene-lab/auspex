mod app;
mod controller;
mod fixtures;
mod omegon_control;
mod remote_session;
mod session_model;

fn main() {
    dioxus::launch(app::App);
}
