use dioxus::prelude::*;

use crate::bootstrap::BootstrapResult;
use crate::controller::{AppController, SessionMode};
use crate::event_stream::EventStreamHandle;
use crate::fixtures::{DevScenario, MessageRole, ShellState};
use crate::screens::{GraphScreen, SessionScreen, WorkScreen};

/// CSS embedded at compile time — bypasses the asset-serving pipeline so
/// the stylesheet is always available in the bundled .app.
const MAIN_CSS: &str = include_str!("../assets/main.css");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Chat,
    Work,
    Graph,
    Session,
}

#[component]
pub fn App() -> Element {
    let bootstrap = try_consume_context::<BootstrapResult>();
    // Extract spawning binary before bootstrap is consumed by use_signal.
    let spawning_binary: Option<String> = bootstrap.as_ref().and_then(|b| {
        if let crate::bootstrap::BootstrapSource::SpawningOmegon { binary } = &b.source {
            Some(binary.clone())
        } else {
            None
        }
    });
    let mut event_stream = use_signal(|| None::<EventStreamHandle>);
    let mut controller = use_signal(move || {
        if let Some(bootstrap) = bootstrap {
            event_stream.set(bootstrap.event_stream);
            let mut controller = bootstrap.controller;
            controller.set_bootstrap_note(bootstrap.note);
            controller
        } else {
            AppController::default()
        }
    });

    use_future(move || {
        let mut controller = controller;
        let event_stream = event_stream;
        async move {
            loop {
                if let Some(handle) = event_stream.read().clone() {
                    let events = handle.inbox.drain();
                    if !events.is_empty() {
                        let mut controller = controller.write();
                        for event in events {
                            let _ = controller.apply_remote_event_json(&event);
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }
        }
    });

    // Async Omegon spawn: handle SpawningOmegon bootstrap source without
    // blocking the UI thread. Updates controller + event_stream on completion.
    use_future(move || {
        let binary = spawning_binary.clone();
        let mut controller = controller;
        let mut event_stream = event_stream;
        async move {
            let Some(binary_str) = binary else { return };
            let binary_path = std::path::PathBuf::from(binary_str);
            let result = tokio::task::spawn_blocking(move || {
                crate::bootstrap::spawn_and_attach_omegon(&binary_path)
            })
            .await
            .expect("spawn task panicked");
            if let Some(stream) = result.event_stream {
                event_stream.set(Some(stream));
            }
            let mut c = result.controller;
            if let Some(note) = result.note {
                c.set_bootstrap_note(Some(note));
            }
            controller.set(c);
        }
    });

    // Auto-scroll transcript to the latest message whenever messages change.
    use_effect(move || {
        let _ = controller.read().messages().len();
        spawn(async move {
            let _ = document::eval(
                r#"
                var el = document.getElementById('transcript-end');
                if (el) el.scrollIntoView({ behavior: 'instant' });
            "#,
            )
            .await;
        });
    });

    let shell_state = controller.read().shell_state();
    let is_fatal = matches!(shell_state, ShellState::Failed);
    let is_starting = matches!(
        shell_state,
        ShellState::Booting | ShellState::StartingStyrene | ShellState::StartingOmegon
    );
    let is_reconnecting = matches!(shell_state, ShellState::CompatibilityChecking);
    let mut tab = use_signal(|| Tab::Chat);

    rsx! {
        document::Style { "{MAIN_CSS}" }
        div { class: "shell",
            header { class: "header",
                div {
                    h1 { "Auspex" }
                    p {
                        if controller.read().is_remote() {
                            "Conversation-first scaffold · remote control-plane projection"
                        } else {
                            "Conversation-first scaffold"
                        }
                    }
                }
                div { class: controller.read().shell_state().status_class(), "{controller.read().shell_state().label()}" }
            }

            section { class: "devbar",
                label { "Source" }
                select {
                    value: controller.read().session_mode().key(),
                    onchange: move |event| controller.write().switch_session_mode(event.value().as_str()),
                    for mode in SessionMode::ALL {
                        option { value: mode.key(), "{mode.label()}" }
                    }
                }

                if !controller.read().is_remote() {
                    label { "Scenario" }
                    select {
                        value: controller.read().scenario().key(),
                        onchange: move |event| controller.write().select_scenario(event.value().as_str()),
                        for scenario in DevScenario::ALL {
                            option { value: scenario.key(), "{scenario.label()}" }
                        }
                    }
                } else {
                    span { class: "devbar-note", "Remote mode is snapshot-driven; scenario overrides are disabled." }
                    if event_stream.read().is_some() {
                        span { class: "devbar-note", "Live WS event stream attached." }
                    }
                }
            }

            if let Some(note) = controller.read().bootstrap_note() {
                section { class: "bootstrap-note",
                    strong { "Bootstrap" }
                    p { "{note}" }
                }
            }

            if is_starting {
                // Startup screen — shown while async Omegon spawn is in progress
                section { class: "state-screen state-screen-starting",
                    div { class: "state-screen-icon", "⏳" }
                    h2 { "{controller.read().shell_state().label()}" }
                    p { class: "state-screen-detail",
                        "Launching the Omegon engine. \
                         The conversation shell will activate automatically once ready."
                    }
                }
            } else {
                // Reconnecting banner — shown when WS dropped but session data is still valid
                if is_reconnecting {
                    section { class: "banner banner-reconnecting",
                        strong { "Reconnecting…" }
                        span { " The connection to the host is being restored. New input is temporarily paused. Cached session state is shown." }
                    }
                }

                section { class: "summary-bar",
                    div { class: "summary-card",
                        h2 { "Connection" }
                        p { "{controller.read().summary().connection}" }
                    }
                    div { class: "summary-card",
                        h2 { "Work" }
                        p { "{controller.read().summary().work}" }
                    }
                }

                // Activity strip — event-driven; dot pulses green while a run is in progress
                section { class: "activity-strip",
                    div {
                        class: if controller.read().is_run_active() {
                            "run-dot run-dot-active"
                        } else {
                            "run-dot run-dot-idle"
                        }
                    }
                    span { class: "activity-label", "{controller.read().summary().activity}" }
                }

                // Power-mode tab bar — only shown in remote mode
                if controller.read().is_remote() {
                    nav { class: "tab-bar",
                        button {
                            class: if *tab.read() == Tab::Chat { "tab tab-active" } else { "tab" },
                            onclick: move |_| tab.set(Tab::Chat),
                            "Chat"
                        }
                        button {
                            class: if *tab.read() == Tab::Work { "tab tab-active" } else { "tab" },
                            onclick: move |_| tab.set(Tab::Work),
                            "Work"
                        }
                        button {
                            class: if *tab.read() == Tab::Graph { "tab tab-active" } else { "tab" },
                            onclick: move |_| tab.set(Tab::Graph),
                            "Graph"
                        }
                        button {
                            class: if *tab.read() == Tab::Session { "tab tab-active" } else { "tab" },
                            onclick: move |_| tab.set(Tab::Session),
                            "Session"
                        }
                    }
                }

                // Fatal startup overlay — blocks normal operation until the
                // embedded backend or explicit remote attach succeeds.
                if is_fatal {
                    section { class: "compat-failure",
                        strong {
                            if controller.read().scenario() == DevScenario::CompatibilityFailure {
                                "Compatibility failure"
                            } else {
                                "Embedded backend startup failed"
                            }
                        }
                        p { "{controller.read().summary().connection}" }
                        p {
                            class: "compat-detail",
                            if controller.read().scenario() == DevScenario::CompatibilityFailure {
                                "Auspex cannot operate with the detected host. Update Omegon to a compatible version and restart."
                            } else {
                                "Auspex requires its embedded Omegon backend for local operation. Fix backend startup and relaunch, or explicitly attach to a remote Omegon control plane."
                            }
                        }
                    }
                } else if *tab.read() == Tab::Work {
                    WorkScreen { data: controller.read().work_data() }
                } else if *tab.read() == Tab::Graph {
                    GraphScreen { data: controller.read().graph_data() }
                } else if *tab.read() == Tab::Session {
                    SessionScreen { data: controller.read().session_data() }
                } else {
                    main { class: "transcript",
                        for message in controller.read().messages().iter() {
                            article {
                                class: match message.role {
                                    MessageRole::User => "bubble bubble-user",
                                    MessageRole::Assistant => "bubble bubble-assistant",
                                    MessageRole::System => "bubble bubble-system",
                                },
                                h2 {
                                    match message.role {
                                        MessageRole::User => "You",
                                        MessageRole::Assistant => "Auspex",
                                        MessageRole::System => "System",
                                    }
                                }
                                p { "{message.text}" }
                            }
                        }
                        div { id: "transcript-end" }
                    }

                    form {
                        class: "composer",
                        onsubmit: move |event| {
                            event.prevent_default();
                            let command = controller.write().submit_prompt_command_json();
                            if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                                stream.send_command(command);
                            }
                        },
                        textarea {
                            class: "composer-input",
                            rows: "3",
                            value: controller.read().composer().draft().to_string(),
                            disabled: !controller.read().can_submit(),
                            placeholder: if controller.read().can_submit() {
                                "Start with the smallest useful prompt…"
                            } else {
                                "Conversation input is unavailable in the current host state."
                            },
                            oninput: move |event| controller.write().update_draft(event.value()),
                            onkeydown: move |event| {
                                if event.key() == Key::Enter
                                    && (event.modifiers().contains(Modifiers::CONTROL)
                                        || event.modifiers().contains(Modifiers::META))
                                {
                                    let command = controller.write().submit_prompt_command_json();
                                    if let (Some(command), Some(stream)) =
                                        (command, event_stream.read().clone())
                                    {
                                        stream.send_command(command);
                                    }
                                }
                            },
                        }
                        div { class: "composer-actions",
                            if controller.read().is_run_active() {
                                button {
                                    class: "composer-cancel",
                                    r#type: "button",
                                    onclick: move |_| {
                                        if let Some(command) = controller.read().cancel_command_json() {
                                            if let Some(stream) = event_stream.read().clone() {
                                                stream.send_command(command);
                                            }
                                        }
                                    },
                                    "Cancel"
                                }
                            }
                            button {
                                class: "composer-submit",
                                r#type: "submit",
                                disabled: !controller.read().can_submit(),
                                "Send"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::controller::AppController;
    use crate::fixtures::*;
    use crate::session_model::HostSessionModel;

    #[test]
    fn blank_draft_does_not_submit() {
        let mut session = MockHostSession::default();
        session.composer_mut().set_draft("   ");

        assert!(!session.submit());
        assert_eq!(session.messages().len(), 1);
        assert_eq!(session.composer().draft(), "   ");
    }

    #[test]
    fn submit_appends_user_and_placeholder_reply() {
        let mut controller = AppController::default();
        controller.update_draft("hello world");

        assert!(controller.submit_prompt());
        assert_eq!(controller.composer().draft(), "");
        assert_eq!(controller.messages().len(), 3);
        assert_eq!(controller.messages()[1].role, MessageRole::User);
        assert_eq!(controller.messages()[1].text, "hello world");
        assert_eq!(controller.messages()[2].role, MessageRole::Assistant);
    }

    #[test]
    fn booting_state_blocks_submit() {
        let mut session = MockHostSession::from_scenario(DevScenario::Booting);
        session.composer_mut().set_draft("hello world");

        assert!(!session.submit());
        assert_eq!(session.messages().len(), 1);
    }

    #[test]
    fn degraded_state_allows_submit() {
        let mut session = MockHostSession::from_scenario(DevScenario::Degraded);
        session.composer_mut().set_draft("still there?");

        assert!(session.submit());
        assert_eq!(session.messages().len(), 4);
    }

    #[test]
    fn reconnecting_state_blocks_submit() {
        let mut session = MockHostSession::from_scenario(DevScenario::Reconnecting);
        session.composer_mut().set_draft("can you hear me?");

        assert!(!session.submit());
        assert_eq!(session.messages().len(), 2);
    }

    #[test]
    fn trait_can_read_core_fields() {
        let controller = AppController::default();
        let model: &dyn HostSessionModel = controller.as_model();

        assert_eq!(model.shell_state(), crate::fixtures::ShellState::Ready);
        assert_eq!(model.scenario(), DevScenario::Ready);
        assert_eq!(model.messages().len(), 1);
    }

    #[test]
    fn remote_demo_controller_exposes_remote_mode() {
        let controller = AppController::remote_demo();

        assert!(controller.is_remote());
        assert!(
            controller
                .summary()
                .connection
                .contains("Attached to Omegon host")
        );
        assert_eq!(controller.messages().len(), 1);
    }
}
