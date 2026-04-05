use dioxus::prelude::*;

use crate::audit_timeline::{AuditEntry, AuditEntryKind, AuditTimelineStore};
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
    Audit,
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
    let mut audit_session_filter = use_signal(String::new);
    let mut audit_turn_filter = use_signal(String::new);
    let mut audit_kind_filter = use_signal(|| "all".to_string());
    let mut audit_text_filter = use_signal(String::new);

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
                        button {
                            class: if *workspace.read() == Workspace::Audit { "tab tab-active" } else { "tab" },
                            onclick: move |_| workspace.set(Workspace::Audit),
                            "Audit"
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
                    section {
                        class: surface.kind.section_class(),
                        "data-surface": app_surface_surface(surface.kind),
                        "data-state": app_surface_state(surface.kind),
                        "data-tone": app_surface_tone(surface.kind),
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
                    section {
                        class: surface.kind.section_class(),
                        "data-surface": app_surface_surface(surface.kind),
                        "data-state": app_surface_state(surface.kind),
                        "data-tone": app_surface_tone(surface.kind),
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
                    section {
                        class: surface.kind.section_class(),
                        "data-surface": app_surface_surface(surface.kind),
                        "data-state": app_surface_state(surface.kind),
                        "data-tone": app_surface_tone(surface.kind),
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
                        controller.read().audit_timeline(),
                    )}
                    WorkScreen { data: controller.read().work_data() }
                    section { class: "rail-section",
                        h2 { class: "rail-heading", "Transcript" }
                        label { class: "rail-toggle",
                            input {
                                r#type: "checkbox",
                                checked: controller.read().transcript_auto_expand(),
                                onchange: move |event| {
                                    controller
                                        .write()
                                        .set_transcript_auto_expand(event.value() == "true");
                                },
                            }
                            div { class: "rail-toggle-copy",
                                span { class: "rail-toggle-title", "Auto-expand transcript details" }
                                span { class: "rail-toggle-detail", "When enabled, short human-readable details open by default." }
                            }
                        }
                    }
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
                    } else if *workspace.read() == Workspace::Audit {
                        {render_audit_workspace(
                            controller.read().audit_timeline(),
                            controller.read().current_audit_session_key().as_str(),
                            AuditPanelControls {
                                filters: AuditFilters {
                                    session_key: audit_session_filter.read().clone(),
                                    turn_query: audit_turn_filter.read().clone(),
                                    kind_key: audit_kind_filter.read().clone(),
                                    text_query: audit_text_filter.read().clone(),
                                },
                                on_session_filter: EventHandler::new(move |value: String| audit_session_filter.set(value)),
                                on_turn_filter: EventHandler::new(move |value: String| audit_turn_filter.set(value)),
                                on_kind_filter: EventHandler::new(move |value: String| audit_kind_filter.set(value)),
                                on_text_filter: EventHandler::new(move |value: String| audit_text_filter.set(value)),
                                on_focus_entry: EventHandler::new(move |target: String| {
                                    focus_transcript_target(controller.read().transcript(), &target);
                                }),
                            },
                        )}
                    } else if *workspace.read() == Workspace::Scribe {
                        ScribeScreen {
                            summary: controller.read().summary().clone(),
                            data: controller.read().session_data(),
                            on_dispatcher_switch: Some(EventHandler::new(move |(profile, model): (String, Option<String>)| {
                                let command = controller.write().request_dispatcher_switch_command_json(&profile, model.as_deref());
                                if let (Some(command), Some(stream)) = (command, event_stream.read().clone()) {
                                    stream.send_command(command);
                                }
                            })),
                            on_transcript_focus: Some(EventHandler::new(move |target: String| {
                                focus_transcript_target(controller.read().transcript(), &target);
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
                                {render_transcript(
                                    controller.read().transcript(),
                                    controller.read().messages(),
                                    controller.read().transcript_auto_expand(),
                                )}
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
                        })),
                        on_transcript_focus: Some(EventHandler::new(move |target: String| {
                            focus_transcript_target(controller.read().transcript(), &target);
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

fn focus_transcript_target(transcript: &TranscriptData, target: &str) {
    if let Some(anchor) = find_transcript_anchor(transcript, target) {
        let anchor_json = serde_json::to_string(&anchor).unwrap_or_else(|_| "\"\"".to_string());
        spawn(async move {
            let _ = document::eval(&format!(
                r#"
                (function() {{
                  var anchor = {anchor_json};
                  if (!anchor) return;
                  var match = document.getElementById(anchor);
                  if (match) {{
                    match.scrollIntoView({{ behavior: 'instant', block: 'center' }});
                    match.classList.add('transcript-focus-hit');
                    setTimeout(function() {{ match.classList.remove('transcript-focus-hit'); }}, 1400);
                  }}
                }})();
                "#
            ))
            .await;
        });
    }
}

fn find_transcript_anchor(transcript: &TranscriptData, target: &str) -> Option<String> {
    for turn in &transcript.turns {
        for (block_index, block) in turn.blocks.iter().enumerate() {
            if transcript_block_matches_target(block, target) {
                return Some(transcript_block_dom_id(turn.number, block_index));
            }
        }
    }
    None
}

fn transcript_block_dom_id(turn_number: u32, block_index: usize) -> String {
    format!("turn-{turn_number}-block-{block_index}")
}

fn transcript_block_matches_target(block: &crate::fixtures::TurnBlock, target: &str) -> bool {
    if let Some(task_id) = target.strip_prefix("delegate:") {
        return match block {
            crate::fixtures::TurnBlock::Text(text) | crate::fixtures::TurnBlock::System(text) => {
                block_origin_label(text.origin.as_ref()).is_some_and(|label| label.contains(task_id))
                    || text.text.contains(task_id)
            }
            _ => false,
        };
    }

    if let Some(token) = target.strip_prefix("dispatcher-switch:") {
        return match block {
            crate::fixtures::TurnBlock::System(text) => {
                text.notice_kind == Some(crate::fixtures::SystemNoticeKind::DispatcherSwitch)
                    && (token.is_empty() || text.text.contains(token))
            }
            _ => false,
        };
    }

    transcript_block_search_text(block).contains(target)
}

fn transcript_block_search_text(block: &crate::fixtures::TurnBlock) -> String {
    match block {
        crate::fixtures::TurnBlock::Thinking(thinking) => thinking.text.clone(),
        crate::fixtures::TurnBlock::Text(text) | crate::fixtures::TurnBlock::System(text) => {
            format!("{} {}", block_origin_label(text.origin.as_ref()).unwrap_or_default(), text.text)
        }
        crate::fixtures::TurnBlock::Tool(tool) => format!(
            "{} {} {} {} {}",
            tool.id,
            tool.name,
            tool.args,
            tool.partial_output,
            tool.result.clone().unwrap_or_default()
        ),
        crate::fixtures::TurnBlock::Aborted(text) => text.clone(),
    }
}

fn block_origin_label(origin: Option<&crate::fixtures::BlockOrigin>) -> Option<&str> {
    origin.map(|origin| origin.label.as_str())
}

const TRANSCRIPT_DISCLOSURE_LINE_THRESHOLD: usize = 7;
const TRANSCRIPT_DISCLOSURE_CHAR_THRESHOLD: usize = 360;
const SYSTEM_NOTICE_DISCLOSURE_LINE_THRESHOLD: usize = 5;
const SYSTEM_NOTICE_DISCLOSURE_CHAR_THRESHOLD: usize = 220;
const STRUCTURED_PAYLOAD_PREFIXES: [&str; 8] = ["{", "[", "(", "<", "---", "diff --", "@@", "{"];

fn transcript_disclosure_meta(content: &str) -> String {
    let line_count = content.lines().count().max(1);
    format!("{line_count} line{} · {} chars", if line_count == 1 { "" } else { "s" }, content.chars().count())
}

fn should_expand_tool_args(content: &str) -> bool {
    should_expand_tool_payload(content)
}

fn should_expand_tool_output(content: &str) -> bool {
    should_expand_tool_payload(content)
}

fn should_expand_system_notice(content: &str) -> bool {
    should_expand_transcript_content(
        content,
        SYSTEM_NOTICE_DISCLOSURE_LINE_THRESHOLD,
        SYSTEM_NOTICE_DISCLOSURE_CHAR_THRESHOLD,
    )
}

fn should_expand_tool_payload(content: &str) -> bool {
    !looks_like_structured_payload(content)
        && !should_expand_transcript_content(
            content,
            TRANSCRIPT_DISCLOSURE_LINE_THRESHOLD,
            TRANSCRIPT_DISCLOSURE_CHAR_THRESHOLD,
        )
}

fn looks_like_structured_payload(content: &str) -> bool {
    let trimmed = content.trim_start();
    if trimmed.is_empty() {
        return false;
    }

    if STRUCTURED_PAYLOAD_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
    {
        return true;
    }

    let first_line = trimmed.lines().next().unwrap_or_default();
    first_line.contains(": ")
        && (first_line.contains('{')
            || first_line.contains('[')
            || first_line.contains("=>")
            || first_line.contains("::"))
}

fn should_expand_transcript_content(content: &str, line_threshold: usize, char_threshold: usize) -> bool {
    content.lines().count() > line_threshold || content.chars().count() > char_threshold
}

fn system_notice_summary_label(text: &crate::fixtures::AttributedText) -> &'static str {
    match text.notice_kind {
        Some(crate::fixtures::SystemNoticeKind::DispatcherSwitch) => "Dispatcher switch notice",
        Some(crate::fixtures::SystemNoticeKind::CleaveStart) => "Cleave start notice",
        Some(crate::fixtures::SystemNoticeKind::CleaveComplete) => "Cleave completion notice",
        Some(crate::fixtures::SystemNoticeKind::ChildStatus) => "Child status notice",
        Some(crate::fixtures::SystemNoticeKind::Failure) => "Failure notice",
        Some(crate::fixtures::SystemNoticeKind::Generic) | None => "System notice",
    }
}

struct TranscriptDisclosure<'a> {
    section_class: &'static str,
    details_class: &'static str,
    summary_class: &'static str,
    summary_label_class: &'static str,
    summary_meta_class: Option<&'static str>,
    body_class: &'static str,
    content_class: &'static str,
    label: &'static str,
    content: &'a str,
    meta: String,
    open: bool,
    copy_label: &'static str,
}

fn render_transcript_disclosure(disclosure: TranscriptDisclosure<'_>) -> Element {
    let TranscriptDisclosure {
        section_class,
        details_class,
        summary_class,
        summary_label_class,
        summary_meta_class,
        body_class,
        content_class,
        label,
        content,
        meta,
        open,
        copy_label,
    } = disclosure;

    rsx! {
        div { class: section_class,
            details {
                class: details_class,
                open,
                summary { class: summary_class,
                    span { class: summary_label_class, "{label}" }
                    if let Some(summary_meta_class) = summary_meta_class {
                        span { class: summary_meta_class, "{meta}" }
                    }
                }
                div { class: body_class,
                    div { class: "transcript-detail-actions",
                        button {
                            id: format!("copy-{}", label.to_lowercase().replace(' ', "-")),
                            class: "transcript-copy-button",
                            r#type: "button",
                            "data-copy-label": format!("Copy {copy_label}"),
                            onclick: {
                                let content = content.to_string();
                                let button_id = format!("copy-{}", label.to_lowercase().replace(' ', "-"));
                                move |_| copy_to_clipboard(&content, &button_id)
                            },
                            "Copy {copy_label}"
                        }
                    }
                    p { class: content_class, "{content}" }
                }
            }
        }
    }
}

fn copy_to_clipboard(text: &str, button_id: &str) {
    let text_json = serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    let button_id_json = serde_json::to_string(button_id).unwrap_or_else(|_| "\"\"".to_string());
    spawn(async move {
        let _ = document::eval(&format!(
            r#"
            (async function() {{
              var text = {text_json};
              var buttonId = {button_id_json};
              if (!text) return;
              var copied = false;
              if (navigator.clipboard && navigator.clipboard.writeText) {{
                try {{
                  await navigator.clipboard.writeText(text);
                  copied = true;
                }} catch (_) {{}}
              }}
              if (!copied) {{
                var area = document.createElement('textarea');
                area.value = text;
                area.setAttribute('readonly', 'readonly');
                area.style.position = 'fixed';
                area.style.opacity = '0';
                document.body.appendChild(area);
                area.select();
                copied = document.execCommand('copy');
                document.body.removeChild(area);
              }}
              if (copied && buttonId) {{
                var button = document.getElementById(buttonId);
                if (button) {{
                  var original = button.getAttribute('data-copy-label') || button.textContent || 'Copy';
                  button.textContent = 'Copied';
                  button.classList.add('transcript-copy-button-copied');
                  setTimeout(function() {{
                    button.textContent = original;
                    button.classList.remove('transcript-copy-button-copied');
                  }}, 1400);
                }}
              }}
            }})();
            "#
        ))
        .await;
    });
}

fn transcript_disclosure_open(open_by_default: bool, auto_expand: bool) -> bool {
    auto_expand && open_by_default
}

fn render_transcript(transcript: &TranscriptData, messages: &[crate::fixtures::ChatMessage], auto_expand: bool) -> Element {
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
                    id: format!("turn-{}", turn.number),
                    "data-surface": "panel",
                    "data-elevation": "1",
                    h2 { class: "turn-title", "Turn {turn.number}" }
                    for (block_index, block) in turn.blocks.iter().enumerate() {
                        match block {
                            crate::fixtures::TurnBlock::Thinking(thinking) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: if thinking.collapsed { "block block-thinking block-collapsed" } else { "block block-thinking" },
                                    "data-surface": "panel",
                                    "data-tone": "muted",
                                    h3 { "Thinking" }
                                    p { "{thinking.text}" }
                                }
                            },
                            crate::fixtures::TurnBlock::Text(text) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: text_block_class(text.origin.as_ref()),
                                    "data-surface": "panel",
                                    "data-tone": text_block_tone(text.origin.as_ref()),
                                    if let Some(origin) = &text.origin {
                                        h3 { class: origin_class(origin), "{origin.label}" }
                                    }
                                    p { "{text.text}" }
                                }
                            },
                            crate::fixtures::TurnBlock::Tool(tool) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: tool_block_class(tool),
                                    "data-surface": "panel",
                                    "data-kind": "tool",
                                    "data-state": tool_visual_state(tool),
                                    "data-tone": tool_block_tone(tool),
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
                                        {render_transcript_disclosure(TranscriptDisclosure {
                                            section_class: "tool-section",
                                            details_class: "tool-details",
                                            summary_class: "tool-summary",
                                            summary_label_class: "tool-summary-label",
                                            summary_meta_class: Some("tool-summary-meta"),
                                            body_class: "tool-detail-body",
                                            content_class: "tool-args",
                                            label: "Args",
                                            content: &tool.args,
                                            meta: transcript_disclosure_meta(&tool.args),
                                            open: transcript_disclosure_open(
                                                should_expand_tool_args(&tool.args),
                                                auto_expand,
                                            ),
                                            copy_label: "args",
                                        })}
                                    }
                                    if !tool.partial_output.is_empty() {
                                        {render_transcript_disclosure(TranscriptDisclosure {
                                            section_class: "tool-section tool-section-stream",
                                            details_class: "tool-details",
                                            summary_class: "tool-summary",
                                            summary_label_class: "tool-summary-label",
                                            summary_meta_class: Some("tool-summary-meta"),
                                            body_class: "tool-detail-body",
                                            content_class: "tool-partial",
                                            label: tool_partial_label(tool),
                                            content: &tool.partial_output,
                                            meta: transcript_disclosure_meta(&tool.partial_output),
                                            open: transcript_disclosure_open(
                                                should_expand_tool_output(&tool.partial_output),
                                                auto_expand,
                                            ),
                                            copy_label: "output",
                                        })}
                                    }
                                    if let Some(result) = &tool.result {
                                        {render_transcript_disclosure(TranscriptDisclosure {
                                            section_class: "tool-section tool-section-result",
                                            details_class: "tool-details",
                                            summary_class: "tool-summary",
                                            summary_label_class: "tool-summary-label",
                                            summary_meta_class: Some("tool-summary-meta"),
                                            body_class: "tool-detail-body",
                                            content_class: "tool-result",
                                            label: tool_result_label(tool),
                                            content: result,
                                            meta: transcript_disclosure_meta(result),
                                            open: transcript_disclosure_open(
                                                should_expand_tool_output(result),
                                                auto_expand,
                                            ),
                                            copy_label: "result",
                                        })}
                                    } else if !tool.is_error {
                                        p { class: "tool-awaiting", "Waiting for final tool result…" }
                                    }
                                }
                            },
                            crate::fixtures::TurnBlock::System(text) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: system_block_class(text),
                                    "data-surface": "panel",
                                    "data-tone": system_block_tone(text),
                                    if let Some(origin) = &text.origin {
                                        h3 { class: origin_class(origin), "{origin.label}" }
                                    }
                                    if should_expand_system_notice(&text.text) {
                                        {render_transcript_disclosure(TranscriptDisclosure {
                                            section_class: "system-section",
                                            details_class: "system-details",
                                            summary_class: "system-summary",
                                            summary_label_class: "system-summary-label",
                                            summary_meta_class: Some("system-summary-meta"),
                                            body_class: "system-detail-body",
                                            content_class: "system-text",
                                            label: system_notice_summary_label(text),
                                            content: &text.text,
                                            meta: transcript_disclosure_meta(&text.text),
                                            open: transcript_disclosure_open(true, auto_expand),
                                            copy_label: "notice",
                                        })}
                                    } else {
                                        p { class: "system-text", "{text.text}" }
                                    }
                                }
                            },
                            crate::fixtures::TurnBlock::Aborted(text) => rsx! {
                                section {
                                    id: transcript_block_dom_id(turn.number, block_index),
                                    class: "block block-aborted",
                                    "data-surface": "panel",
                                    "data-tone": "danger",
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
            "data-tone": chat_status_tone(is_run_active, can_submit),
            "data-activity-kind": activity_kind,
            title: "Activity: {activity_kind}",
            strong { class: "chat-status-label", "{label}" }
            span { class: "chat-status-detail", "{detail}" }
        }
    }
}

fn app_surface_surface(kind: crate::fixtures::AppSurfaceKind) -> &'static str {
    match kind {
        crate::fixtures::AppSurfaceKind::Startup => "panel",
        crate::fixtures::AppSurfaceKind::Reconnecting => "banner",
        crate::fixtures::AppSurfaceKind::StartupFailure
        | crate::fixtures::AppSurfaceKind::CompatibilityFailure => "panel",
        crate::fixtures::AppSurfaceKind::BootstrapNote => "panel",
    }
}

fn app_surface_state(kind: crate::fixtures::AppSurfaceKind) -> &'static str {
    match kind {
        crate::fixtures::AppSurfaceKind::Startup => "starting",
        crate::fixtures::AppSurfaceKind::Reconnecting => "reconnecting",
        crate::fixtures::AppSurfaceKind::StartupFailure => "startup-failure",
        crate::fixtures::AppSurfaceKind::CompatibilityFailure => "compatibility-failure",
        crate::fixtures::AppSurfaceKind::BootstrapNote => "info",
    }
}

fn app_surface_tone(kind: crate::fixtures::AppSurfaceKind) -> &'static str {
    match kind {
        crate::fixtures::AppSurfaceKind::Startup | crate::fixtures::AppSurfaceKind::BootstrapNote => "info",
        crate::fixtures::AppSurfaceKind::Reconnecting => "warn",
        crate::fixtures::AppSurfaceKind::StartupFailure
        | crate::fixtures::AppSurfaceKind::CompatibilityFailure => "danger",
    }
}

fn chat_status_tone(is_run_active: bool, can_submit: bool) -> &'static str {
    if is_run_active {
        "info"
    } else if !can_submit {
        "warn"
    } else {
        "success"
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

fn text_block_tone(origin: Option<&crate::fixtures::BlockOrigin>) -> &'static str {
    match origin.map(|origin| &origin.kind) {
        Some(crate::fixtures::OriginKind::Child) => "accent",
        Some(crate::fixtures::OriginKind::System) => "muted",
        Some(crate::fixtures::OriginKind::Dispatcher) | None => "default",
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

fn system_block_tone(text: &crate::fixtures::AttributedText) -> &'static str {
    match text.notice_kind {
        Some(crate::fixtures::SystemNoticeKind::Failure) => "danger",
        Some(crate::fixtures::SystemNoticeKind::CleaveStart)
        | Some(crate::fixtures::SystemNoticeKind::DispatcherSwitch) => "info",
        Some(crate::fixtures::SystemNoticeKind::CleaveComplete) => "success",
        Some(crate::fixtures::SystemNoticeKind::ChildStatus) => "accent",
        Some(crate::fixtures::SystemNoticeKind::Generic) | None => {
            text_block_tone(text.origin.as_ref())
        }
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

fn tool_block_tone(tool: &crate::fixtures::ToolCard) -> &'static str {
    if tool.is_error {
        "danger"
    } else if tool.result.is_some() {
        "success"
    } else if tool.partial_output.is_empty() {
        "muted"
    } else {
        "info"
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AuditFilters {
    session_key: String,
    turn_query: String,
    kind_key: String,
    text_query: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AuditPanelEntry {
    heading: String,
    meta: String,
    content: String,
    kind_key: &'static str,
    focus_target: Option<String>,
}

#[derive(Clone, Debug)]
struct AuditPanelControls {
    filters: AuditFilters,
    on_session_filter: EventHandler<String>,
    on_turn_filter: EventHandler<String>,
    on_kind_filter: EventHandler<String>,
    on_text_filter: EventHandler<String>,
    on_focus_entry: EventHandler<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AuditPanelModel {
    total_count: usize,
    filtered_count: usize,
    latest_label: String,
    session_options: Vec<String>,
    entries: Vec<AuditPanelEntry>,
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
    audit_timeline: &AuditTimelineStore,
) -> Element {
    let inventory = build_left_rail_inventory(summary, work, session, is_run_active);
    let audit_count = audit_timeline.entries.len();
    let latest_audit_label = audit_timeline
        .entries
        .last()
        .map(|entry| entry.label.as_str())
        .unwrap_or("No transcript blocks retained yet");

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
        section { class: "rail-section",
            h2 { class: "rail-heading", "Audit" }
            div { class: "rail-card audit-summary-card",
                div { class: "rail-card-title", "{audit_count} retained block(s)" }
                p { class: "rail-card-detail", "Latest: {latest_audit_label}" }
            }
        }
    }
}

fn render_audit_workspace(
    audit_timeline: &AuditTimelineStore,
    current_audit_session_key: &str,
    controls: AuditPanelControls,
) -> Element {
    let audit_panel = build_audit_panel_model(
        audit_timeline,
        current_audit_session_key,
        &controls.filters,
    );

    rsx! {
        div { class: "screen screen-audit",
            section { class: "screen-section",
                h2 { class: "screen-section-title", "Audit history" }
                p { class: "screen-empty",
                    "Project-wide retained transcript blocks across sessions. Filter by session, turn, kind, or text; jump to live transcript blocks when they belong to the current session."
                }
            }
            div { class: "audit-workspace-layout",
                section { class: "screen-section audit-panel audit-panel-controls",
                    div { class: "rail-card audit-summary-card",
                        div { class: "rail-card-title", "{audit_panel.filtered_count} of {audit_panel.total_count} retained block(s)" }
                        p { class: "rail-card-detail", "Latest: {audit_panel.latest_label}" }
                    }
                    div { class: "rail-list audit-filter-list",
                        label { class: "audit-filter-field",
                            span { class: "audit-filter-label", "Session" }
                            select {
                                class: "audit-filter-control",
                                value: controls.filters.session_key.clone(),
                                onchange: move |event| controls.on_session_filter.call(event.value()),
                                option { value: "", "All sessions" }
                                for session_key in &audit_panel.session_options {
                                    option { value: session_key.clone(), "{session_key}" }
                                }
                            }
                        }
                        label { class: "audit-filter-field",
                            span { class: "audit-filter-label", "Turn" }
                            input {
                                class: "audit-filter-control",
                                r#type: "search",
                                inputmode: "numeric",
                                placeholder: "All turns",
                                value: controls.filters.turn_query.clone(),
                                oninput: move |event| controls.on_turn_filter.call(event.value()),
                            }
                        }
                        label { class: "audit-filter-field",
                            span { class: "audit-filter-label", "Kind" }
                            select {
                                class: "audit-filter-control",
                                value: controls.filters.kind_key.clone(),
                                onchange: move |event| controls.on_kind_filter.call(event.value()),
                                for (value, label) in audit_kind_options() {
                                    option { value: value, "{label}" }
                                }
                            }
                        }
                        label { class: "audit-filter-field",
                            span { class: "audit-filter-label", "Search" }
                            input {
                                class: "audit-filter-control",
                                r#type: "search",
                                placeholder: "Label or retained text",
                                value: controls.filters.text_query.clone(),
                                oninput: move |event| controls.on_text_filter.call(event.value()),
                            }
                        }
                    }
                }
                section { class: "screen-section audit-panel audit-panel-results",
                    div { class: "audit-entry-list audit-entry-list-workspace",
                        if audit_panel.entries.is_empty() {
                            p { class: "rail-placeholder", "No retained transcript blocks match the current filters." }
                        } else {
                            for entry in &audit_panel.entries {
                                article {
                                    class: "audit-entry-card",
                                    "data-kind": entry.kind_key,
                                    h3 { class: "audit-entry-title", "{entry.heading}" }
                                    p { class: "audit-entry-meta", "{entry.meta}" }
                                    if let Some(target) = &entry.focus_target {
                                        button {
                                            class: "audit-entry-jump",
                                            r#type: "button",
                                            onclick: {
                                                let target = target.clone();
                                                let handler = controls.on_focus_entry;
                                                move |_| handler.call(target.clone())
                                            },
                                            "Jump to transcript"
                                        }
                                    } else {
                                        p { class: "audit-entry-unavailable", "Transcript block not present in the current session." }
                                    }
                                    pre { class: "audit-entry-content", "{entry.content}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn audit_kind_options() -> [(&'static str, &'static str); 6] {
    [
        ("all", "All kinds"),
        ("thinking", "Thinking"),
        ("text", "Message"),
        ("tool", "Tool"),
        ("system", "System"),
        ("aborted", "Aborted"),
    ]
}

fn build_audit_panel_model(
    audit_timeline: &AuditTimelineStore,
    current_audit_session_key: &str,
    filters: &AuditFilters,
) -> AuditPanelModel {
    let latest_label = audit_timeline
        .entries
        .last()
        .map(|entry| entry.label.clone())
        .unwrap_or_else(|| "No transcript blocks retained yet".into());
    let mut session_options = audit_timeline
        .entries
        .iter()
        .map(|entry| entry.session_key.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    session_options.sort();

    let entries = audit_timeline
        .entries
        .iter()
        .filter(|entry| audit_entry_matches_filters(entry, filters))
        .map(|entry| AuditPanelEntry {
            heading: entry.label.clone(),
            meta: format!(
                "{} · turn {} · {}",
                entry.session_key,
                entry.turn_number,
                audit_kind_label(entry.kind.clone())
            ),
            content: entry.content.clone(),
            kind_key: audit_kind_key(entry.kind.clone()),
            focus_target: (entry.session_key == current_audit_session_key)
                .then(|| transcript_block_dom_id(entry.turn_number, entry.block_index)),
        })
        .collect::<Vec<_>>();

    AuditPanelModel {
        total_count: audit_timeline.entries.len(),
        filtered_count: entries.len(),
        latest_label,
        session_options,
        entries,
    }
}

fn audit_entry_matches_filters(entry: &AuditEntry, filters: &AuditFilters) -> bool {
    let session_filter = filters.session_key.trim();
    if !session_filter.is_empty() && entry.session_key != session_filter {
        return false;
    }

    let turn_filter = filters.turn_query.trim();
    if !turn_filter.is_empty() {
        let Ok(turn_number) = turn_filter.parse::<u32>() else {
            return false;
        };
        if entry.turn_number != turn_number {
            return false;
        }
    }

    let kind_filter = filters.kind_key.trim();
    if !kind_filter.is_empty() && kind_filter != "all" && audit_kind_key(entry.kind.clone()) != kind_filter {
        return false;
    }

    let text_filter = filters.text_query.trim().to_ascii_lowercase();
    if !text_filter.is_empty() {
        let haystack = format!(
            "{}\n{}\n{}\n{}",
            entry.session_key,
            entry.turn_number,
            entry.label,
            entry.content,
        )
        .to_ascii_lowercase();
        if !haystack.contains(&text_filter) {
            return false;
        }
    }

    true
}

fn audit_kind_key(kind: AuditEntryKind) -> &'static str {
    match kind {
        AuditEntryKind::Thinking => "thinking",
        AuditEntryKind::Text => "text",
        AuditEntryKind::Tool => "tool",
        AuditEntryKind::System => "system",
        AuditEntryKind::Aborted => "aborted",
    }
}

fn audit_kind_label(kind: AuditEntryKind) -> &'static str {
    match kind {
        AuditEntryKind::Thinking => "Thinking",
        AuditEntryKind::Text => "Message",
        AuditEntryKind::Tool => "Tool",
        AuditEntryKind::System => "System",
        AuditEntryKind::Aborted => "Aborted",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        app_surface_state, app_surface_tone, audit_entry_matches_filters,
        audit_kind_key, build_audit_panel_model, block_origin_label,
        build_left_rail_inventory, chat_status_tone, find_transcript_anchor,
        looks_like_structured_payload, render_chat_status_banner,
        transcript_disclosure_open,
        should_expand_system_notice, should_expand_tool_args,
        should_expand_tool_output, system_block_class, system_block_tone,
        system_notice_summary_label, text_block_class, text_block_tone,
        tool_block_class, tool_block_tone, tool_partial_label,
        tool_result_label, tool_status_label, tool_visual_state,
        transcript_block_dom_id, transcript_disclosure_meta, AuditFilters,
    };
    use crate::audit_timeline::{AuditEntry, AuditEntryKind, AuditTimelineStore};
    use crate::controller::AppController;
    use crate::session_model::HostSessionModel;
    use crate::fixtures::{
        ActivityKind, AttributedText, BlockOrigin, DevScenario, HostSessionSummary,
        MessageRole, MockHostSession, OriginKind, SystemNoticeKind, ToolCard,
        TranscriptData,
    };

    #[test]
    fn transcript_anchor_lookup_matches_delegate_and_dispatcher_switch_targets() {
        let transcript = TranscriptData {
            turns: vec![crate::fixtures::Turn {
                number: 7,
                blocks: vec![
                    crate::fixtures::TurnBlock::System(AttributedText {
                        text: "Dispatcher switch confirmed (dispatcher-switch-9): supervisor-heavy · openai:gpt-4.1".into(),
                        origin: Some(BlockOrigin {
                            kind: OriginKind::Dispatcher,
                            label: "anthropic:claude-sonnet-4-6".into(),
                        }),
                        notice_kind: Some(SystemNoticeKind::DispatcherSwitch),
                    }),
                    crate::fixtures::TurnBlock::System(AttributedText {
                        text: "Cleave child child-b completed successfully".into(),
                        origin: Some(BlockOrigin {
                            kind: OriginKind::Child,
                            label: "Child child-b".into(),
                        }),
                        notice_kind: Some(SystemNoticeKind::ChildStatus),
                    }),
                ],
            }],
            active_turn: None,
            context_tokens: None,
        };

        assert_eq!(transcript_block_dom_id(7, 1), "turn-7-block-1");
        assert_eq!(
            find_transcript_anchor(&transcript, "delegate:child-b").as_deref(),
            Some("turn-7-block-1")
        );
        assert_eq!(
            find_transcript_anchor(&transcript, "dispatcher-switch:dispatcher-switch-9").as_deref(),
            Some("turn-7-block-0")
        );
        assert_eq!(block_origin_label(None), None);
    }

    #[test]
    fn text_block_class_keeps_dispatcher_text_as_normal_text() {
        let origin = BlockOrigin {
            kind: OriginKind::Dispatcher,
            label: "anthropic:claude-sonnet-4-6".into(),
        };

        assert_eq!(text_block_class(Some(&origin)), "block block-text");
        assert_eq!(text_block_tone(Some(&origin)), "default");
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
        assert_eq!(system_block_tone(&dispatcher_failure), "danger");
        assert_eq!(
            system_block_class(&child_failure),
            "block block-system block-child-origin block-control-failure"
        );
        assert_eq!(system_block_tone(&child_failure), "danger");
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
        assert_eq!(tool_block_tone(&queued), "muted");
        assert_eq!(tool_block_tone(&streaming), "info");
        assert_eq!(tool_block_tone(&complete), "success");
        assert_eq!(tool_block_tone(&errored), "danger");
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
    fn transcript_disclosure_helpers_expand_only_verbose_or_human_readable_content() {
        let short = "echo hi";
        let structured_json = r#"{"cmd":"cargo test","cwd":"/tmp/project"}"#;
        let structured_yaml = "---\nname: release\nintent: cut rc";
        let long_lines = (1..=8)
            .map(|i| format!("line-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let long_chars = "x".repeat(361);

        assert!(should_expand_tool_args(short));
        assert!(should_expand_tool_output(short));
        assert!(!looks_like_structured_payload(short));
        assert!(looks_like_structured_payload(structured_json));
        assert!(looks_like_structured_payload(structured_yaml));
        assert!(!should_expand_tool_args(structured_json));
        assert!(!should_expand_tool_output(structured_yaml));
        assert!(!should_expand_tool_args(&long_lines));
        assert!(!should_expand_tool_output(&long_chars));
        assert_eq!(transcript_disclosure_meta("alpha\nbeta"), "2 lines · 10 chars");
    }

    #[test]
    fn transcript_disclosure_open_respects_operator_setting() {
        assert!(transcript_disclosure_open(true, true));
        assert!(!transcript_disclosure_open(true, false));
        assert!(!transcript_disclosure_open(false, true));
    }

    #[test]
    fn left_rail_inventory_audit_summary_uses_latest_entry() {
        let controller = AppController::default();
        let panel = build_audit_panel_model(
            controller.audit_timeline(),
            &controller.current_audit_session_key(),
            &AuditFilters::default(),
        );

        assert_eq!(panel.total_count, 0);
        assert_eq!(panel.filtered_count, 0);
        assert_eq!(panel.latest_label, "No transcript blocks retained yet");
    }

    #[test]
    fn audit_panel_filters_by_session_turn_kind_and_text() {
        let store = AuditTimelineStore::from_json(
            r#"{
              "schema_version": 1,
              "entries": [
                {
                  "session_key": "remote:a",
                  "turn_number": 4,
                  "block_index": 0,
                  "block_id": "remote:a:turn-4-block-0",
                  "kind": "System",
                  "label": "Dispatcher",
                  "content": "Switch confirmed"
                },
                {
                  "session_key": "remote:b",
                  "turn_number": 9,
                  "block_index": 1,
                  "block_id": "remote:b:turn-9-block-1",
                  "kind": "Tool",
                  "label": "Tool · bash",
                  "content": "cargo test"
                }
              ]
            }"#,
        )
        .expect("audit timeline fixture should deserialize");

        let filters = AuditFilters {
            session_key: "remote:b".into(),
            turn_query: "9".into(),
            kind_key: "tool".into(),
            text_query: "cargo".into(),
        };
        let panel = build_audit_panel_model(&store, "remote:b", &filters);

        assert_eq!(panel.total_count, 2);
        assert_eq!(panel.filtered_count, 1);
        assert_eq!(panel.session_options, vec!["remote:a".to_string(), "remote:b".to_string()]);
        assert_eq!(panel.entries[0].heading, "Tool · bash");
        assert_eq!(panel.entries[0].kind_key, "tool");
        assert_eq!(panel.entries[0].meta, "remote:b · turn 9 · Tool");
        assert_eq!(panel.entries[0].focus_target.as_deref(), Some("turn-9-block-1"));
    }

    #[test]
    fn audit_panel_only_offers_focus_for_current_session_entries() {
        let store = AuditTimelineStore::from_json(
            r#"{
              "schema_version": 1,
              "entries": [
                {
                  "session_key": "remote:a",
                  "turn_number": 4,
                  "block_index": 0,
                  "block_id": "remote:a:turn-4-block-0",
                  "kind": "System",
                  "label": "Dispatcher",
                  "content": "Switch confirmed"
                }
              ]
            }"#,
        )
        .expect("audit timeline fixture should deserialize");

        let panel = build_audit_panel_model(&store, "remote:b", &AuditFilters::default());
        assert_eq!(panel.entries[0].focus_target, None);
    }

    #[test]
    fn audit_entry_filter_rejects_invalid_turn_query() {
        let entry = AuditEntry {
            session_key: "remote:a".into(),
            turn_number: 3,
            block_index: 0,
            block_id: "remote:a:turn-3-block-0".into(),
            kind: AuditEntryKind::Thinking,
            label: "Thinking".into(),
            content: "inspect".into(),
        };

        assert!(!audit_entry_matches_filters(
            &entry,
            &AuditFilters {
                turn_query: "three".into(),
                ..AuditFilters::default()
            }
        ));
        assert_eq!(audit_kind_key(AuditEntryKind::Thinking), "thinking");
    }

    #[test]
    fn system_notice_disclosure_and_labels_follow_notice_kind() {
        let short_generic = AttributedText {
            text: "Background refresh complete".into(),
            origin: None,
            notice_kind: Some(SystemNoticeKind::Generic),
        };
        let long_failure = AttributedText {
            text: (1..=6)
                .map(|i| format!("failure detail {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            origin: Some(BlockOrigin {
                kind: OriginKind::Dispatcher,
                label: "anthropic:claude-sonnet-4-6".into(),
            }),
            notice_kind: Some(SystemNoticeKind::Failure),
        };

        assert!(!should_expand_system_notice(&short_generic.text));
        assert!(should_expand_system_notice(&long_failure.text));
        assert_eq!(system_notice_summary_label(&short_generic), "System notice");
        assert_eq!(system_notice_summary_label(&long_failure), "Failure notice");
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
        assert_eq!(chat_status_tone(true, false), "info");
        assert_eq!(chat_status_tone(false, false), "warn");
        assert_eq!(chat_status_tone(false, true), "success");
    }

    #[test]
    fn app_surface_helpers_assign_semantic_state_and_tone() {
        use crate::fixtures::AppSurfaceKind;

        assert_eq!(app_surface_state(AppSurfaceKind::Startup), "starting");
        assert_eq!(app_surface_tone(AppSurfaceKind::Startup), "info");
        assert_eq!(app_surface_state(AppSurfaceKind::Reconnecting), "reconnecting");
        assert_eq!(app_surface_tone(AppSurfaceKind::Reconnecting), "warn");
        assert_eq!(
            app_surface_state(AppSurfaceKind::CompatibilityFailure),
            "compatibility-failure"
        );
        assert_eq!(app_surface_tone(AppSurfaceKind::CompatibilityFailure), "danger");
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
