use dioxus::prelude::*;

use crate::bootstrap::BootstrapResult;
use crate::controller::{AppController, SessionMode};
use crate::event_stream::EventStreamHandle;
use crate::fixtures::{DevScenario, MessageRole, TranscriptData};
use crate::screens::{GraphScreen, ScribeScreen, SessionScreen, WorkScreen};

/// CSS embedded at compile time — bypasses the asset-serving pipeline so
/// the stylesheet is always available in the bundled .app.
const MAIN_CSS: &str = include_str!("../assets/main.css");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Workspace {
    Chat,
    Scribe,
    Graph,
}

#[component]
pub fn App() -> Element {
    let bootstrap = try_consume_context::<BootstrapResult>();
    // Extract spawning binary before bootstrap is consumed by use_signal.
    #[cfg(not(target_arch = "wasm32"))]
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
                #[cfg(not(target_arch = "wasm32"))]
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                #[cfg(target_arch = "wasm32")]
                gloo_timers::future::TimeoutFuture::new(150).await;
            }
        }
    });

    // Async Omegon spawn: desktop-only.
    #[cfg(not(target_arch = "wasm32"))]
    use_future(move || {
        let binary = spawning_binary.clone();
        let mut controller = controller;
        let mut event_stream = event_stream;
        async move {
            let Some(binary_str) = binary else { return };
            let binary_path = std::path::PathBuf::from(binary_str);
            let result =
                crate::bootstrap::spawn_and_attach_omegon(&binary_path).await;
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

    let mut workspace = use_signal(|| Workspace::Chat);

    let session = controller.read().session_data();
    let bootstrap_surface = controller
        .read()
        .surface_notice()
        .filter(|surface| surface.kind == crate::fixtures::AppSurfaceKind::BootstrapNote);
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

            div { class: "top-chrome",
                // ── Top bar ─────────────────────────────────────────────
                header { class: "topbar",
                    // Top-left corner box — shell identity
                    div { class: "topbar-identity",
                        h1 { class: "topbar-title", "Auspex" }
                        span { class: "topbar-subtitle",
                            if controller.read().is_remote() {
                                "Remote"
                            } else {
                                "Local"
                            }
                        }
                        span { class: "version-chip", "v{APP_VERSION}" }
                    }

                    // Top-center — workspace tabs (always visible)
                    nav { class: "topbar-tabs",
                        button {
                            class: if *workspace.read() == Workspace::Chat { "tab tab-active" } else { "tab" },
                            onclick: move |_| workspace.set(Workspace::Chat),
                            "Chat"
                        }
                        button {
                            class: if *workspace.read() == Workspace::Scribe { "tab tab-active" } else { "tab" },
                            onclick: move |_| workspace.set(Workspace::Scribe),
                            "Scribe"
                        }
                        button {
                            class: if *workspace.read() == Workspace::Graph { "tab tab-active" } else { "tab" },
                            onclick: move |_| workspace.set(Workspace::Graph),
                            "Graph"
                        }
                    }

                    // Top-right — global state
                    div { class: "topbar-status",
                        if let Some(surface) = bootstrap_surface.as_ref() {
                            span {
                                class: "topbar-meta",
                                title: "{surface.body}",
                                "{surface.body}"
                            }
                        }
                        div { class: controller.read().shell_state().status_class(), "{controller.read().shell_state().label()}" }
                    }
                }

                // ── Surface notices that still deserve dedicated space ──
                if let Some(surface) = controller.read().surface_notice()
                    && surface.kind == crate::fixtures::AppSurfaceKind::Startup
                {
                    section { class: surface.kind.section_class(),
                        div { class: "state-screen-icon", "⏳" }
                        h2 { "{surface.kind.title()}" }
                        p { class: "state-screen-detail", "{surface.body}" }
                        if let Some(detail) = surface.detail.as_deref() {
                            p { class: "state-screen-detail", "{detail}" }
                        }
                    }
                }

                if let Some(surface) = controller.read().surface_notice()
                    && surface.kind == crate::fixtures::AppSurfaceKind::Reconnecting
                {
                    section { class: surface.kind.section_class(),
                        strong { "{surface.kind.title()}" }
                        span { " {surface.body}" }
                    }
                }

                if let Some(surface) = controller.read().surface_notice()
                    && matches!(
                        surface.kind,
                        crate::fixtures::AppSurfaceKind::StartupFailure
                            | crate::fixtures::AppSurfaceKind::CompatibilityFailure
                    )
                {
                    section { class: surface.kind.section_class(),
                        strong { "{surface.kind.title()}" }
                        p { "{surface.body}" }
                        if let Some(detail) = surface.detail.as_deref() {
                            p { class: "compat-detail", "{detail}" }
                        }
                    }
                }
            }

            // ── Main area: left rail / center / right rail ─────────────
            div { class: "main-area",

                // Left rail — project/session navigator
                aside { class: "left-rail",
                    {render_left_rail_inventory(
                        controller.read().summary(),
                        &controller.read().work_data(),
                        &controller.read().session_data(),
                        controller.read().is_run_active(),
                    )}
                    WorkScreen { data: controller.read().work_data() }
                    // Dev controls — temporary, will move to a proper settings surface
                    section { class: "rail-section rail-devbar",
                        h2 { class: "rail-heading", "Dev" }
                        select {
                            value: controller.read().session_mode().key(),
                            onchange: move |event| controller.write().switch_session_mode(event.value().as_str()),
                            for mode in SessionMode::ALL {
                                option { value: mode.key(), "{mode.label()}" }
                            }
                        }
                        if !controller.read().is_remote() {
                            select {
                                value: controller.read().scenario().key(),
                                onchange: move |event| controller.write().select_scenario(event.value().as_str()),
                                for scenario in DevScenario::ALL {
                                    option { value: scenario.key(), "{scenario.label()}" }
                                }
                            }
                        }
                    }
                }

                // Center workspace — active tab content
                div { class: "center-workspace",
                    if *workspace.read() == Workspace::Graph {
                        GraphScreen { data: controller.read().graph_data() }
                    } else if *workspace.read() == Workspace::Scribe {
                        ScribeScreen {
                            summary: controller.read().summary().clone(),
                            data: controller.read().session_data(),
                            on_dispatcher_switch: Some(EventHandler::new(move |(profile, model): (String, Option<String>)| {
                                let command = controller.write().request_dispatcher_switch_command_json(&profile, model.as_deref());
                                if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                                    stream.send_command(command);
                                }
                            }))
                        }
                    } else {
                        // Chat workspace — transcript + composer
                        div { class: "chat-workspace",
                            {render_chat_status_banner(
                                controller.read().summary(),
                                controller.read().is_run_active(),
                                controller.read().can_submit(),
                            )}
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

                // Right rail — contextual inspector
                aside { class: "right-rail",
                    SessionScreen {
                        data: controller.read().session_data(),
                        on_dispatcher_switch: Some(EventHandler::new(move |(profile, model): (String, Option<String>)| {
                            let command = controller.write().request_dispatcher_switch_command_json(&profile, model.as_deref());
                            if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                                stream.send_command(command);
                            }
                        }))
                    }
                }
            }

            // ── Bottom bar ──────────────────────────────────────────────
            footer { class: "bottombar",
                // Bottom-left corner box — org/operator identity
                div { class: "bottombar-org",
                    span { class: "bottombar-label", "Operator" }
                }

                // Bottom-center — instrumentation
                div { class: "bottombar-instruments",
                    span { class: "instrument", "{controller.read().summary().connection}" }
                    span { class: "instrument", "{context_status}" }
                }

                // Bottom-right corner box — reserved aperture
                div { class: "bottombar-reserved" }
            }
        }
    }
}

fn render_transcript(transcript: &TranscriptData, messages: &[crate::fixtures::ChatMessage]) -> Element {
    if transcript.turns.is_empty() && messages.len() <= 2 {
        rsx! {
            section { class: "chat-empty-state",
                h2 { "Ready for first contact" }
                p {
                    "Auspex is attached. Start with a small directive, a status question, or a dispatcher/profile change if you need a different operator posture."
                }
                ul { class: "chat-empty-list",
                    li { "Summarize the current host session and any active work." }
                    li { "Inspect dispatcher status and tell me whether a switch is needed." }
                    li { "Plan the next implementation step from the focused design state." }
                }
            }
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
    } else if transcript.turns.is_empty() {
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
                article {
                    class: "turn-card",
                    "data-surface": "panel",
                    "data-elevation": "1",
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
                                section {
                                    class: tool_block_class(tool),
                                    "data-surface": "panel",
                                    "data-kind": "tool",
                                    "data-state": tool_visual_state(tool),
                                    div { class: "tool-header",
                                        div { class: "tool-header-main",
                                            if let Some(origin) = &tool.origin {
                                                h3 { class: origin_class(origin), "{origin.label}" }
                                            }
                                            h3 { class: "tool-name", "{tool.name}" }
                                        }
                                        span { class: tool_status_class(tool), "{tool_status_label(tool)}" }
                                    }
                                    if !tool.args.is_empty() {
                                        div { class: "tool-section",
                                            span { class: "tool-section-label", "Args" }
                                            p { class: "tool-args", "{tool.args}" }
                                        }
                                    }
                                    if !tool.partial_output.is_empty() {
                                        div { class: "tool-section tool-section-stream",
                                            span { class: "tool-section-label", "{tool_partial_label(tool)}" }
                                            p { class: "tool-partial", "{tool.partial_output}" }
                                        }
                                    }
                                    if let Some(result) = &tool.result {
                                        div { class: "tool-section tool-section-result",
                                            span { class: "tool-section-label", "{tool_result_label(tool)}" }
                                            p { class: "tool-result", "{result}" }
                                        }
                                    } else if !tool.is_error {
                                        p { class: "tool-awaiting", "Waiting for final tool result…" }
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

fn render_chat_status_banner(
    summary: &crate::fixtures::HostSessionSummary,
    is_run_active: bool,
    can_submit: bool,
) -> Element {
    let (banner_class, banner_state, label, detail) = if is_run_active {
        (
            "chat-status-banner chat-status-banner-running",
            "running",
            "Run active",
            "Omegon is working. New input is paused until the current run completes or you cancel it.",
        )
    } else if !can_submit {
        (
            "chat-status-banner chat-status-banner-blocked",
            "blocked",
            "Input paused",
            "Conversation input is unavailable in the current host state.",
        )
    } else {
        (
            "chat-status-banner",
            "ready",
            "Ready",
            summary.activity.as_str(),
        )
    };

    let activity_kind = summary.activity_kind.label();

    rsx! {
        section {
            class: banner_class,
            "data-surface": "banner",
            "data-state": banner_state,
            "data-activity-kind": activity_kind,
            title: "Activity: {activity_kind}",
            strong { class: "chat-status-label", "{label}" }
            span { class: "chat-status-detail", "{detail}" }
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
    match text.notice_kind {
        Some(crate::fixtures::SystemNoticeKind::DispatcherSwitch) => {
            "block block-system block-dispatcher-system"
        }
        Some(crate::fixtures::SystemNoticeKind::CleaveStart) => {
            "block block-system block-dispatcher-system block-control-cleave"
        }
        Some(crate::fixtures::SystemNoticeKind::CleaveComplete) => {
            "block block-system block-dispatcher-system block-control-complete"
        }
        Some(crate::fixtures::SystemNoticeKind::ChildStatus) => {
            "block block-system block-child-origin block-control-child"
        }
        Some(crate::fixtures::SystemNoticeKind::Failure) => {
            match text.origin.as_ref().map(|origin| &origin.kind) {
                Some(crate::fixtures::OriginKind::Child) => {
                    "block block-system block-child-origin block-control-failure"
                }
                Some(crate::fixtures::OriginKind::Dispatcher) => {
                    "block block-system block-dispatcher-system block-control-failure"
                }
                _ => "block block-system block-control-failure",
            }
        }
        Some(crate::fixtures::SystemNoticeKind::Generic) | None => match text
            .origin
            .as_ref()
            .map(|origin| &origin.kind)
        {
            Some(crate::fixtures::OriginKind::Dispatcher) => "block block-system block-dispatcher-system",
            Some(crate::fixtures::OriginKind::Child) => "block block-system block-child-origin",
            Some(crate::fixtures::OriginKind::System) => "block block-system",
            None => "block block-system",
        },
    }
}

fn tool_block_class(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "block block-tool block-error"
    } else if tool.result.is_some() {
        "block block-tool block-tool-complete"
    } else {
        "block block-tool block-tool-running"
    }
}

fn tool_status_class(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "tool-status tool-status-error"
    } else if tool.result.is_some() {
        "tool-status tool-status-complete"
    } else {
        "tool-status tool-status-running"
    }
}

fn tool_visual_state(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "error"
    } else if tool.result.is_some() {
        "complete"
    } else if tool.partial_output.is_empty() {
        "queued"
    } else {
        "streaming"
    }
}

fn tool_status_label(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "Error"
    } else if tool.result.is_some() {
        "Complete"
    } else if tool.partial_output.is_empty() {
        "Queued"
    } else {
        "Streaming"
    }
}

fn tool_partial_label(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.result.is_some() {
        "Streamed output"
    } else {
        "Live output"
    }
}

fn tool_result_label(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "Error result"
    } else {
        "Final result"
    }
}

fn origin_class(origin: &crate::fixtures::BlockOrigin) -> &'static str {
    match origin.kind {
        crate::fixtures::OriginKind::Dispatcher => "block-origin block-origin-dispatcher",
        crate::fixtures::OriginKind::Child => "block-origin block-origin-child",
        crate::fixtures::OriginKind::System => "block-origin block-origin-system",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LeftRailInventory {
    workspace_label: String,
    project_label: String,
    session_label: String,
    session_detail: String,
    agent_rows: Vec<(String, String)>,
}

fn build_left_rail_inventory(
    summary: &crate::fixtures::HostSessionSummary,
    work: &crate::fixtures::WorkData,
    session: &crate::fixtures::SessionData,
    is_run_active: bool,
) -> LeftRailInventory {
    let workspace_label = session
        .git_branch
        .clone()
        .unwrap_or_else(|| "detached".into());
    let project_label = work
        .focused_title
        .clone()
        .or_else(|| Some(summary.work.clone()))
        .unwrap_or_else(|| "No focused work item".into());

    let session_label = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| binding.session_id.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "local-session".into());

    let session_detail = session
        .dispatcher_binding
        .as_ref()
        .map(|binding| {
            binding
                .expected_model
                .clone()
                .unwrap_or_else(|| binding.expected_profile.clone())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| summary.connection.clone());

    let mut agent_rows = Vec::new();
    if let Some(binding) = session.dispatcher_binding.as_ref() {
        let dispatcher_name = if let Some(model) = binding.expected_model.as_ref() {
            format!("Dispatcher · {model}")
        } else if !binding.dispatcher_instance_id.is_empty() {
            format!("Dispatcher · {}", binding.dispatcher_instance_id)
        } else {
            format!("Dispatcher · {}", binding.expected_profile)
        };
        let dispatcher_status = if is_run_active {
            "running".to_string()
        } else {
            binding.expected_role.clone()
        };
        agent_rows.push((dispatcher_name, dispatcher_status));
    }

    for delegate in &session.active_delegates {
        agent_rows.push((
            format!("Delegate · {}", delegate.agent_name),
            format!("{} · {} ms", delegate.status, delegate.elapsed_ms),
        ));
    }

    if agent_rows.is_empty() {
        agent_rows.push(("Dispatcher · unavailable".into(), "idle".into()));
    }

    LeftRailInventory {
        workspace_label,
        project_label,
        session_label,
        session_detail,
        agent_rows,
    }
}

fn render_left_rail_inventory(
    summary: &crate::fixtures::HostSessionSummary,
    work: &crate::fixtures::WorkData,
    session: &crate::fixtures::SessionData,
    is_run_active: bool,
) -> Element {
    let inventory = build_left_rail_inventory(summary, work, session, is_run_active);

    rsx! {
        section { class: "rail-section",
            h2 { class: "rail-heading", "Workspace" }
            div { class: "rail-card",
                div { class: "rail-card-title", "{inventory.workspace_label}" }
                p { class: "rail-card-detail", "{inventory.project_label}" }
            }
        }
        section { class: "rail-section",
            h2 { class: "rail-heading", "Session" }
            div { class: "rail-card",
                div { class: "rail-card-title", "{inventory.session_label}" }
                p { class: "rail-card-detail", "{inventory.session_detail}" }
            }
        }
        section { class: "rail-section",
            h2 { class: "rail-heading", "Agents" }
            div { class: "rail-list",
                for (name, detail) in &inventory.agent_rows {
                    div { class: "rail-list-item",
                        span { class: "rail-list-title", "{name}" }
                        span { class: "rail-list-detail", "{detail}" }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_left_rail_inventory, render_chat_status_banner, system_block_class,
        text_block_class, tool_block_class, tool_partial_label, tool_result_label,
        tool_status_label, tool_visual_state,
    };
    use crate::controller::AppController;
    use crate::session_model::HostSessionModel;
    use crate::fixtures::{
        ActivityKind, AttributedText, BlockOrigin, DevScenario, HostSessionSummary,
        MessageRole, MockHostSession, OriginKind, SystemNoticeKind, ToolCard,
    };

    #[test]
    fn text_block_class_keeps_dispatcher_text_as_normal_text() {
        let origin = BlockOrigin {
            kind: OriginKind::Dispatcher,
            label: "anthropic:claude-sonnet-4-6".into(),
        };

        assert_eq!(text_block_class(Some(&origin)), "block block-text");
    }

    #[test]
    fn system_block_class_marks_dispatcher_switch_notices_distinctly() {
        let text = AttributedText {
            text: "Dispatcher switch confirmed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::DispatcherSwitch),
        };

        assert_eq!(
            system_block_class(&text),
            "block block-system block-dispatcher-system"
        );
    }

    #[test]
    fn system_block_class_marks_cleave_notices_from_notice_kind() {
        let start = AttributedText {
            text: "Dispatcher requested decomposition into 2 child task(s)".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::CleaveStart),
        };
        let complete = AttributedText {
            text: "Dispatcher completed decomposition and merged child results".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::CleaveComplete),
        };

        assert_eq!(
            system_block_class(&start),
            "block block-system block-dispatcher-system block-control-cleave"
        );
        assert_eq!(
            system_block_class(&complete),
            "block block-system block-dispatcher-system block-control-complete"
        );
    }

    #[test]
    fn system_block_class_marks_failures_from_notice_kind() {
        let dispatcher_failure = AttributedText {
            text: "Dispatcher switch failed (dispatcher-switch-1): supervisor-heavy · openai:gpt-4.1 [backend_rejected]".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::Failure),
        };
        let child_failure = AttributedText {
            text: "Cleave child child-b failed".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Child,
                label: "Child child-b".into(),
            }),
            notice_kind: Some(SystemNoticeKind::Failure),
        };

        assert_eq!(
            system_block_class(&dispatcher_failure),
            "block block-system block-dispatcher-system block-control-failure"
        );
        assert_eq!(
            system_block_class(&child_failure),
            "block block-system block-child-origin block-control-failure"
        );
    }

    #[test]
    fn system_block_class_marks_child_status_from_notice_kind() {
        let text = AttributedText {
            text: "Cleave child child-a completed successfully".into(),
            origin: Some(BlockOrigin {
                kind: OriginKind::Child,
                label: "Child child-a".into(),
            }),
            notice_kind: Some(SystemNoticeKind::ChildStatus),
        };

        assert_eq!(
            system_block_class(&text),
            "block block-system block-child-origin block-control-child"
        );
    }

    #[test]
    fn tool_status_label_distinguishes_queued_streaming_complete_and_error() {
        let queued = ToolCard {
            id: "tool-1".into(),
            name: "bash".into(),
            args: String::new(),
            partial_output: String::new(),
            result: None,
            is_error: false,
            origin: None,
        };
        let streaming = ToolCard {
            partial_output: "compiling…".into(),
            ..queued.clone()
        };
        let complete = ToolCard {
            result: Some("done".into()),
            ..streaming.clone()
        };
        let errored = ToolCard {
            is_error: true,
            result: Some("boom".into()),
            ..queued.clone()
        };

        assert_eq!(tool_status_label(&queued), "Queued");
        assert_eq!(tool_status_label(&streaming), "Streaming");
        assert_eq!(tool_status_label(&complete), "Complete");
        assert_eq!(tool_status_label(&errored), "Error");
        assert_eq!(tool_visual_state(&queued), "queued");
        assert_eq!(tool_visual_state(&streaming), "streaming");
        assert_eq!(tool_visual_state(&complete), "complete");
        assert_eq!(tool_visual_state(&errored), "error");
    }

    #[test]
    fn tool_labels_and_classes_reflect_partial_and_final_output_state() {
        let running = ToolCard {
            id: "tool-1".into(),
            name: "bash".into(),
            args: "echo hi".into(),
            partial_output: "hi".into(),
            result: None,
            is_error: false,
            origin: None,
        };
        let complete = ToolCard {
            result: Some("status 0".into()),
            ..running.clone()
        };
        let errored = ToolCard {
            is_error: true,
            result: Some("status 1".into()),
            ..running.clone()
        };

        assert_eq!(tool_block_class(&running), "block block-tool block-tool-running");
        assert_eq!(tool_block_class(&complete), "block block-tool block-tool-complete");
        assert_eq!(tool_block_class(&errored), "block block-tool block-error");
        assert_eq!(tool_partial_label(&running), "Live output");
        assert_eq!(tool_partial_label(&complete), "Streamed output");
        assert_eq!(tool_result_label(&complete), "Final result");
        assert_eq!(tool_result_label(&errored), "Error result");
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
    fn chat_status_banner_reports_run_state() {
        let summary = HostSessionSummary {
            connection: "Attached".into(),
            activity: "Waiting for input".into(),
            activity_kind: ActivityKind::Idle,
            work: "No focused work".into(),
        };
        let banner = render_chat_status_banner(&summary, true, false);
        let debug = format!("{banner:?}");

        assert!(debug.contains("Run active"));
        assert!(debug.contains("current run completes"));
    }

    #[test]
    fn left_rail_inventory_prefers_dispatcher_and_delegate_state() {
        let controller = AppController::remote_demo();
        let inventory = build_left_rail_inventory(
            controller.summary(),
            &controller.work_data(),
            &controller.session_data(),
            controller.is_run_active(),
        );

        assert_eq!(inventory.workspace_label, "main");
        assert_eq!(inventory.session_label, "session_01HVDEMO");
        assert!(inventory.agent_rows[0].0.contains("Dispatcher"));
    }

    #[test]
    fn left_rail_inventory_falls_back_when_dispatcher_absent() {
        let controller = AppController::default();
        let inventory = build_left_rail_inventory(
            controller.summary(),
            &controller.work_data(),
            &controller.session_data(),
            controller.is_run_active(),
        );

        assert_eq!(inventory.workspace_label, "main");
        assert_eq!(inventory.session_label, "local-session");
        assert_eq!(inventory.agent_rows[0].0, "Dispatcher · unavailable");
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
