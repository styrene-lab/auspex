use dioxus::prelude::*;

use crate::bootstrap::BootstrapResult;
use crate::controller::{AppController, SessionMode};
use crate::event_stream::EventStreamHandle;
use crate::fixtures::{DevScenario, MessageRole, ShellState, TranscriptData};
use crate::screens::{GraphScreen, SessionScreen, WorkScreen};

/// CSS embedded at compile time — bypasses the asset-serving pipeline so
/// the stylesheet is always available in the bundled .app.
const MAIN_CSS: &str = include_str!("../assets/main.css");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    let is_starting = matches!(shell_state, ShellState::StartingOmegon);
    let is_reconnecting = matches!(shell_state, ShellState::CompatibilityChecking);
    let mut tab = use_signal(|| Tab::Chat);

    let session = controller.read().session_data();
    let context_status = if let Some(tokens) = session.context_tokens {
        if let Some(window) = session.context_window {
            format!("{tokens} / {window} tokens")
        } else {
            format!("{tokens} tokens")
        }
    } else {
        "No context usage reported".to_string()
    };

    rsx! {
        document::Style { "{MAIN_CSS}" }
        div { class: "shell",
            header { class: "header",
                div { class: "header-copy",
                    h1 { "Auspex" }
                    p {
                        if controller.read().is_remote() {
                            "Connected to Omegon control plane"
                        } else {
                            "Offline mock mode"
                        }
                    }
                }
                div { class: "header-meta",
                    span { class: "version-chip", "v{APP_VERSION}" }
                    div { class: controller.read().shell_state().status_class(), "{controller.read().shell_state().label()}" }
                }
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
                        h2 { "Context" }
                        p { "{context_status}" }
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
                    SessionScreen {
                        data: controller.read().session_data(),
                        on_dispatcher_switch: move |(profile, model): (String, Option<String>)| {
                            let command = controller
                                .write()
                                .request_dispatcher_switch_command_json(&profile, model.as_deref());
                            if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                                stream.send_command(command);
                            }
                        },
                    }
                } else {
                    main { class: "transcript",
                        {render_transcript(controller.read().transcript(), controller.read().messages())}
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
                                        if let Some(command) = controller.read().cancel_command_json()
                                            && let Some(stream) = event_stream.read().clone()
                                        {
                                            stream.send_command(command);
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

fn render_transcript(transcript: &TranscriptData, messages: &[crate::fixtures::ChatMessage]) -> Element {
    if transcript.turns.is_empty() {
        rsx! {
            for message in messages.iter() {
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
        }
    } else {
        rsx! {
            for turn in &transcript.turns {
                article { class: "turn-card",
                    h2 { class: "turn-title", "Turn {turn.number}" }
                    for block in &turn.blocks {
                        match block {
                            crate::fixtures::TurnBlock::Thinking(thinking) => rsx! {
                                section { class: if thinking.collapsed { "block block-thinking block-collapsed" } else { "block block-thinking" },
                                    h3 { "Thinking" }
                                    p { "{thinking.text}" }
                                }
                            },
                            crate::fixtures::TurnBlock::Text(text) => rsx! {
                                section { class: text_block_class(text.origin.as_ref()),
                                    if let Some(origin) = &text.origin {
                                        h3 { class: origin_class(origin), "{origin.label}" }
                                    }
                                    p { "{text.text}" }
                                }
                            },
                            crate::fixtures::TurnBlock::Tool(tool) => rsx! {
                                section { class: if tool.is_error { "block block-tool block-error" } else { "block block-tool" },
                                    if let Some(origin) = &tool.origin {
                                        h3 { class: origin_class(origin), "{origin.label}" }
                                    }
                                    h3 { "{tool.name}" }
                                    p { class: "tool-args", "{tool.args}" }
                                    if !tool.partial_output.is_empty() {
                                        p { class: "tool-partial", "{tool.partial_output}" }
                                    }
                                    if let Some(result) = &tool.result {
                                        p { class: "tool-result", "{result}" }
                                    }
                                }
                            },
                            crate::fixtures::TurnBlock::System(text) => rsx! {
                                section { class: system_block_class(text),
                                    if let Some(origin) = &text.origin {
                                        h3 { class: origin_class(origin), "{origin.label}" }
                                    }
                                    p { "{text.text}" }
                                }
                            },
                            crate::fixtures::TurnBlock::Aborted(text) => rsx! {
                                section { class: "block block-aborted",
                                    p { "{text}" }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

fn text_block_class(origin: Option<&crate::fixtures::BlockOrigin>) -> &'static str {
    match origin.map(|origin| &origin.kind) {
        Some(crate::fixtures::OriginKind::Dispatcher) => "block block-text",
        Some(crate::fixtures::OriginKind::Child) => "block block-system block-child-origin",
        Some(crate::fixtures::OriginKind::System) => "block block-system",
        None => "block block-text",
    }
}

fn system_block_class(text: &crate::fixtures::AttributedText) -> &'static str {
    match text.origin.as_ref().map(|origin| &origin.kind) {
        Some(crate::fixtures::OriginKind::Dispatcher) => {
            dispatcher_system_block_class(text.text.as_str())
        }
        Some(crate::fixtures::OriginKind::Child) => child_system_block_class(text.text.as_str()),
        Some(crate::fixtures::OriginKind::System) => "block block-system",
        None => "block block-system",
    }
}

fn dispatcher_system_block_class(text: &str) -> &'static str {
    if text.contains("switch failed") {
        "block block-system block-dispatcher-system block-control-failure"
    } else if text.contains("requested decomposition") {
        "block block-system block-dispatcher-system block-control-cleave"
    } else if text.contains("completed decomposition") {
        "block block-system block-dispatcher-system block-control-complete"
    } else {
        "block block-system block-dispatcher-system"
    }
}

fn child_system_block_class(text: &str) -> &'static str {
    if text.contains("failed") {
        "block block-system block-child-origin block-control-failure"
    } else {
        "block block-system block-child-origin block-control-child"
    }
}

fn origin_class(origin: &crate::fixtures::BlockOrigin) -> &'static str {
    match origin.kind {
        crate::fixtures::OriginKind::Dispatcher => "block-origin block-origin-dispatcher",
        crate::fixtures::OriginKind::Child => "block-origin block-origin-child",
        crate::fixtures::OriginKind::System => "block-origin block-origin-system",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        child_system_block_class, dispatcher_system_block_class, system_block_class,
        text_block_class,
    };
    use crate::controller::AppController;
    use crate::fixtures::*;
    use crate::session_model::HostSessionModel;

    #[test]
    fn text_block_class_keeps_dispatcher_text_as_normal_text() {
        let origin = BlockOrigin {
            kind: OriginKind::Dispatcher,
            label: "anthropic:claude-sonnet-4-6".into(),
        };

        assert_eq!(text_block_class(Some(&origin)), "block block-text");
    }

    #[test]
    fn system_block_class_marks_dispatcher_notices_distinctly() {
        let text = AttributedText {
            text: "Dispatcher switch confirmed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
        };

        assert_eq!(
            system_block_class(&text),
            "block block-system block-dispatcher-system"
        );
    }

    #[test]
    fn dispatcher_system_block_class_marks_decomposition_notices() {
        assert_eq!(
            dispatcher_system_block_class("Dispatcher requested decomposition into 2 child task(s)"),
            "block block-system block-dispatcher-system block-control-cleave"
        );
        assert_eq!(
            dispatcher_system_block_class("Dispatcher completed decomposition and merged child results"),
            "block block-system block-dispatcher-system block-control-complete"
        );
    }

    #[test]
    fn dispatcher_system_block_class_marks_switch_failures() {
        assert_eq!(
            dispatcher_system_block_class(
                "Dispatcher switch failed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1 [backend_rejected]"
            ),
            "block block-system block-dispatcher-system block-control-failure"
        );
    }

    #[test]
    fn child_system_block_class_distinguishes_success_and_failure() {
        assert_eq!(
            child_system_block_class("Cleave child child-a completed successfully"),
            "block block-system block-child-origin block-control-child"
        );
        assert_eq!(
            child_system_block_class("Cleave child child-b failed"),
            "block block-system block-child-origin block-control-failure"
        );
    }

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
