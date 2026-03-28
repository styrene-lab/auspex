mod app;
mod controller;
mod fixtures;
mod session_model;

fn main() {
    dioxus::launch(app::App);
}
