/// Power-mode Work and Session screens.
///
/// These are composed from `WorkData` and `SessionData` view-models
/// derived from the Omegon snapshot; no additional backend calls needed.
///
/// Some components here are not yet wired into the desktop shell frame
/// but are tested and ready for right-rail / session inspector integration.
use dioxus::prelude::*;

use crate::fixtures::{
    DispatcherBindingData, DispatcherOptionData, DispatcherSwitchStateData, GraphData,
    HostSessionSummary, SessionData, WorkData,
};

// ── Graph screen ──────────────────────────────────────────────────────────────

#[component]
pub fn GraphScreen(data: GraphData) -> Element {
    // Group nodes by status for display
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    let mut seen: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for node in &data.nodes {
        seen.entry(node.status.clone())
            .or_default()
            .push(node.title.clone());
    }
    // Render in a meaningful order
    for status in &[
        "implementing",
        "decided",
        "actionable",
        "exploring",
        "seed",
        "blocked",
    ] {
        if let Some(titles) = seen.remove(*status) {
            groups.push((status.to_string(), titles));
        }
    }
    // Remaining statuses not in the priority list
    let mut rest: Vec<_> = seen.into_iter().collect();
    rest.sort_by(|a, b| a.0.cmp(&b.0));
    groups.extend(rest);

    rsx! {
        div { class: "screen screen-graph",
            if !data.is_full_inventory && !data.nodes.is_empty() {
                p { class: "graph-partial-notice",
                    "Showing implementing and actionable nodes only. A full inventory "
                    "will be available once Omegon exposes the /api/graph endpoint."
                }
            }

            if !data.counts.is_empty() {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Counts" }
                    div { class: "graph-counts",
                        for (status, count) in &data.counts {
                            div {
                                class: "graph-count-chip",
                                "data-surface": "panel",
                                "data-state": status_badge_state(status),
                                "data-tone": status_badge_tone(status),
                                span { class: status_badge_class(status), "{status}" }
                                span { class: "graph-count-num", "{count}" }
                            }
                        }
                    }
                }
            }

            if groups.is_empty() {
                p { class: "screen-empty", "No design-tree nodes in snapshot." }
            } else {
                for (status, titles) in &groups {
                    section { class: "screen-section",
                        h2 { class: "screen-section-title",
                            span { class: status_badge_class(status), "{status}" }
                            " ({titles.len()})"
                        }
                        div { class: "graph-node-list",
                            for title in titles {
                                div { class: "graph-node-row", "{title}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Work screen ──────────────────────────────────────────────────────────────

#[component]
pub fn WorkScreen(data: WorkData) -> Element {
    rsx! {
        div { class: "screen screen-work",

            // Focused node
            section { class: "screen-section",
                h2 { class: "screen-section-title", "Focused" }
                if let Some(title) = &data.focused_title {
                    div { class: "work-focused-card",
                        "data-surface": "panel",
                        "data-state": data.focused_status.as_deref().map(status_badge_state).unwrap_or("idle"),
                        "data-tone": data.focused_status.as_deref().map(status_badge_tone).unwrap_or("muted"),
                        div { class: "work-focused-header",
                            span { class: "work-focused-title", "{title}" }
                            if let Some(status) = &data.focused_status {
                                span {
                                    class: status_badge_class(status),
                                    "{status}"
                                }
                            }
                        }
                        if data.open_question_count > 0 {
                            p {
                                class: "work-focused-meta",
                                "data-tone": "warn",
                                "⚠ {data.open_question_count} open question(s)"
                            }
                        }
                    }
                } else {
                    p { class: "screen-empty", "No node is focused." }
                }
            }

            // Implementing
            if !data.implementing.is_empty() {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Implementing" }
                    for node in &data.implementing {
                        div { class: "work-node-row",
                            span { class: "work-node-title", "{node.title}" }
                            span { class: status_badge_class(&node.status), "{node.status}" }
                        }
                    }
                }
            }

            // Actionable
            if !data.actionable.is_empty() {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Actionable" }
                    for node in &data.actionable {
                        div { class: "work-node-row",
                            span { class: "work-node-title", "{node.title}" }
                            span { class: status_badge_class(&node.status), "{node.status}" }
                        }
                    }
                }
            }

            // OpenSpec + Cleave
            section { class: "screen-section",
                h2 { class: "screen-section-title", "Progress" }
                div { class: "progress-grid",
                    if data.openspec_total > 0 {
                        div { class: "progress-card",
                            span { class: "progress-label", "OpenSpec" }
                            span { class: "progress-value",
                                "{data.openspec_done} / {data.openspec_total}"
                            }
                        }
                    }
                    if data.cleave_total > 0 {
                        div { class: "progress-card",
                            span { class: "progress-label",
                                if data.cleave_active { "Cleave (active)" } else { "Cleave" }
                            }
                            span {
                                class: if data.cleave_failed > 0 { "progress-value progress-value-warn" } else { "progress-value" },
                                "{data.cleave_completed} / {data.cleave_total}"
                                if data.cleave_failed > 0 {
                                    " ({data.cleave_failed} failed)"
                                }
                            }
                        }
                    }
                    if data.openspec_total == 0 && data.cleave_total == 0 {
                        p { class: "screen-empty", "No tracked progress." }
                    }
                }
            }
        }
    }
}

// ── Scribe screen ─────────────────────────────────────────────────────────────

#[component]
pub fn ScribeScreen(
    summary: HostSessionSummary,
    data: SessionData,
    on_dispatcher_switch: Option<EventHandler<(String, Option<String>)>>,
    on_transcript_focus: Option<EventHandler<String>>,
) -> Element {
    let control_summary = session_control_summary(&data);
    let session_alerts = session_alerts(&data);

    rsx! {
        div { class: "screen screen-scribe",
            section { class: "screen-section",
                h2 { class: "screen-section-title", "Scribe" }
                p { class: "screen-empty",
                    "The first-party Rust-native extension surface is not implemented yet, but this workspace now exposes the live operator contract around the current host session."
                }
            }

            section { class: "screen-section",
                h2 { class: "screen-section-title", "Current host" }
                div { class: "kv-grid",
                    {kv_row("Connection", &summary.connection)}
                    {kv_row("Activity", &summary.activity)}
                    {kv_row("Work", &summary.work)}
                    if let Some(dispatcher) = &data.dispatcher_binding {
                        {kv_row("Session", &dispatcher.session_id)}
                        {kv_row("Instance", &dispatcher.dispatcher_instance_id)}
                    }
                }
            }

            section { class: "screen-section",
                h2 { class: "screen-section-title", "Session controls" }
                div { class: "progress-grid progress-grid-tight",
                    for item in &control_summary {
                        div {
                            class: "progress-card progress-card-emphasis",
                            "data-surface": "panel",
                            "data-elevation": "1",
                            "data-tone": session_control_item_tone(item.label),
                            span { class: "progress-label", "{item.label}" }
                            span {
                                class: if item.compact { "progress-value progress-value-small" } else { "progress-value" },
                                "{item.value}"
                            }
                        }
                    }
                }
                if !session_alerts.is_empty() {
                    div { class: "session-alert-list",
                        for alert in &session_alerts {
                            div {
                                class: alert.class_name,
                                "data-surface": "panel",
                                "data-tone": alert.tone,
                                strong { class: "session-alert-title", "{alert.title}" }
                                p { class: "session-alert-body", "{alert.body}" }
                            }
                        }
                    }
                }
            }

            if let Some(dispatcher) = &data.dispatcher_binding {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Dispatcher binding" }
                    div { class: "kv-grid",
                        {kv_row("Canonical session", &dispatcher.session_id)}
                        {kv_row("Canonical instance", &dispatcher.dispatcher_instance_id)}
                        {kv_row("Workspace target", &dispatcher.expected_profile)}
                        if let Some(model) = dispatcher.expected_model.as_deref() {
                            {kv_row("Runtime target", model)}
                        }
                        {kv_row("Control-plane role", &dispatcher.expected_role)}
                        {kv_row("Control-plane schema", &dispatcher.control_plane_schema.to_string())}
                        if let Some(base_url) = dispatcher.observed_base_url.as_deref() {
                            {kv_row("Observed endpoint", base_url)}
                        }
                        if let Some(verified_at) = dispatcher.last_verified_at.as_deref() {
                            {kv_row("Last verified", verified_at)}
                        }
                        if let Some(token_ref) = dispatcher.token_ref.as_deref() {
                            {kv_row("Control-plane token", token_ref)}
                        }
                    }
                }

                if let Some(state) = &dispatcher.switch_state {
                    {render_dispatcher_switch_state(dispatcher, state, on_transcript_focus)}
                }

                if !dispatcher.available_options.is_empty() {
                    section { class: "screen-section",
                        h2 { class: "screen-section-title", "Available bindings" }
                        if let Some(handler) = on_dispatcher_switch {
                            div { class: "dispatcher-option-list",
                                for option in &dispatcher.available_options {
                                    button {
                                        class: dispatcher_option_button_class(dispatcher, option),
                                        "data-surface": "control",
                                        "data-state": dispatcher_option_visual_state(dispatcher, option),
                                        r#type: "button",
                                        disabled: dispatcher_option_disabled(dispatcher, option),
                                        onclick: {
                                            let profile = option.profile.clone();
                                            let model = option.model.clone();
                                            move |_| handler.call((profile.clone(), model.clone()))
                                        },
                                        strong { "{option.label}" }
                                        span { class: "dispatcher-option-meta",
                                            "{option.profile}"
                                            if let Some(model) = &option.model {
                                                " · {model}"
                                            }
                                        }
                                        if let Some(status) = dispatcher_option_status_text(dispatcher, option) {
                                            span { class: "dispatcher-option-status", "{status}" }
                                        }
                                    }
                                }
                            }
                        } else {
                            div { class: "dispatcher-option-list",
                                for option in &dispatcher.available_options {
                                    div {
                                        class: dispatcher_option_button_class(dispatcher, option),
                                        "data-surface": "control",
                                        "data-state": dispatcher_option_visual_state(dispatcher, option),
                                        strong { "{option.label}" }
                                        span { class: "dispatcher-option-meta",
                                            "{option.profile}"
                                            if let Some(model) = &option.model {
                                                " · {model}"
                                            }
                                        }
                                        if let Some(status) = dispatcher_option_status_text(dispatcher, option) {
                                            span { class: "dispatcher-option-status", "{status}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Dispatcher binding" }
                    p { class: "screen-empty", "No dispatcher control-plane binding is present in the current snapshot." }
                }
            }

            if !data.active_delegates.is_empty() {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Active delegates" }
                    div { class: "delegate-list",
                        for delegate in &data.active_delegates {
                            button {
                                class: "delegate-row delegate-row-button",
                                r#type: "button",
                                onclick: {
                                    let task_id = delegate.task_id.clone();
                                    let handler = on_transcript_focus;
                                    move |_| {
                                        if let Some(handler) = &handler {
                                            handler.call(format!("delegate:{task_id}"));
                                        }
                                    }
                                },
                                div { class: "delegate-main",
                                    strong { class: "delegate-title", "{delegate.task_id}" }
                                    span { class: "delegate-agent", "{delegate.agent_name}" }
                                }
                                div { class: "delegate-meta",
                                    span { class: status_badge_class(&delegate.status), "{delegate.status}" }
                                    span { class: "delegate-elapsed", "{format_elapsed_ms(delegate.elapsed_ms)}" }
                                }
                            }
                        }
                    }
                }
            }

            section { class: "screen-section",
                h2 { class: "screen-section-title", "Session health" }
                div { class: "progress-grid",
                    div { class: "progress-card",
                        span { class: "progress-label", "Turns" }
                        span { class: "progress-value", "{data.session_turns}" }
                    }
                    div { class: "progress-card",
                        span { class: "progress-label", "Tool calls" }
                        span { class: "progress-value", "{data.session_tool_calls}" }
                    }
                    div { class: "progress-card",
                        span { class: "progress-label", "Compactions" }
                        span { class: "progress-value", "{data.session_compactions}" }
                    }
                    if let Some(context_usage) = format_context_usage(data.context_tokens, data.context_window) {
                        div { class: "progress-card",
                            span { class: "progress-label", "Context" }
                            span { class: "progress-value progress-value-small", "{context_usage}" }
                        }
                    }
                }
            }
        }
    }
}

// ── Session screen ────────────────────────────────────────────────────────────

#[component]
pub fn SessionScreen(
    data: SessionData,
    on_dispatcher_switch: Option<EventHandler<(String, Option<String>)>>,
    on_transcript_focus: Option<EventHandler<String>>,
) -> Element {
    rsx! {
        div { class: "screen screen-session",

            // Harness
            section { class: "screen-section",
                h2 { class: "screen-section-title", "Harness" }
                div { class: "kv-grid",
                    {kv_row("Branch",
                        data.git_branch.as_deref().unwrap_or("—")
                    )}
                    {kv_row("Thinking",   &data.thinking_level)}
                    {kv_row("Tier",       &data.capability_tier)}
                    {kv_row("Memory",     if data.memory_available { "available" } else { "unavailable" })}
                    {kv_row("Cleave",     if data.cleave_available { "available" } else { "unavailable" })}
                    if let Some(warn) = &data.memory_warning {
                        div { class: "kv-row kv-row-warn",
                            span { class: "kv-key", "Warning" }
                            span { class: "kv-value kv-warn", "{warn}" }
                        }
                    }
                }
            }

            // Providers
            section { class: "screen-section",
                h2 { class: "screen-section-title", "Providers" }
                if data.providers.is_empty() {
                    p { class: "screen-empty", "No provider data." }
                } else {
                    div { class: "kv-grid",
                        for p in &data.providers {
                            div { class: "kv-row",
                                span { class: "kv-key", "{p.name}" }
                                span { class: "kv-value",
                                    if let Some(model) = &p.model {
                                        "{model}"
                                    } else if p.authenticated {
                                        "authenticated"
                                    } else {
                                        "not authenticated"
                                    }
                                    if !p.authenticated { " ⚠" }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(dispatcher) = &data.dispatcher_binding {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Dispatcher" }
                    div { class: "kv-grid",
                        {kv_row("Canonical session", &dispatcher.session_id)}
                        {kv_row("Canonical instance", &dispatcher.dispatcher_instance_id)}
                        {kv_row("Control-plane role", &dispatcher.expected_role)}
                        {kv_row("Workspace target", &dispatcher.expected_profile)}
                        if let Some(model) = &dispatcher.expected_model {
                            {kv_row("Runtime target", model)}
                        }
                        {kv_row("Control-plane schema", &dispatcher.control_plane_schema.to_string())}
                        if let Some(base_url) = &dispatcher.observed_base_url {
                            {kv_row("Observed endpoint", base_url)}
                        }
                        if let Some(last_verified_at) = &dispatcher.last_verified_at {
                            {kv_row("Last verified", last_verified_at)}
                        }
                    }

                    if !dispatcher.available_options.is_empty() {
                        section { class: "screen-subsection",
                            h3 { class: "screen-section-title", "Available dispatcher options" }
                            if let Some(handler) = on_dispatcher_switch {
                                div { class: "dispatcher-option-list",
                                    for option in &dispatcher.available_options {
                                        button {
                                            class: dispatcher_option_button_class(dispatcher, option),
                                            r#type: "button",
                                            disabled: dispatcher_option_disabled(dispatcher, option),
                                            onclick: {
                                                let profile = option.profile.clone();
                                                let model = option.model.clone();
                                                move |_| handler.call((profile.clone(), model.clone()))
                                            },
                                            strong { "{option.label}" }
                                            span {
                                                class: "dispatcher-option-meta",
                                                "{option.profile}"
                                                if let Some(model) = &option.model {
                                                    " · {model}"
                                                }
                                            }
                                            if let Some(status) = dispatcher_option_status_text(dispatcher, option) {
                                                span { class: "dispatcher-option-status", "{status}" }
                                            }
                                        }
                                    }
                                }
                            } else {
                                div { class: "kv-grid",
                                    for option in &dispatcher.available_options {
                                        div { class: "kv-row",
                                            span { class: "kv-key", "{option.label}" }
                                            span { class: "kv-value",
                                                "{option.profile}"
                                                if let Some(model) = &option.model {
                                                    " · {model}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(state) = &dispatcher.switch_state {
                        {render_dispatcher_switch_state(dispatcher, state, on_transcript_focus)}
                    }
                }
            }

            // Active delegates
            if !data.active_delegates.is_empty() {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Active delegates" }
                    div { class: "kv-grid",
                        for delegate in &data.active_delegates {
                            button {
                                class: "kv-row kv-row-button",
                                r#type: "button",
                                onclick: {
                                    let task_id = delegate.task_id.clone();
                                    let handler = on_transcript_focus;
                                    move |_| {
                                        if let Some(handler) = &handler {
                                            handler.call(format!("delegate:{task_id}"));
                                        }
                                    }
                                },
                                span { class: "kv-key", "{delegate.agent_name}" }
                                span { class: "kv-value",
                                    "{delegate.status} · {delegate.task_id} · {delegate.elapsed_ms} ms"
                                }
                            }
                        }
                    }
                }
            }

            // Session stats
            section { class: "screen-section",
                h2 { class: "screen-section-title", "Session stats" }
                div { class: "kv-grid",
                    {kv_row("Turns",       &data.session_turns.to_string())}
                    {kv_row("Tool calls",  &data.session_tool_calls.to_string())}
                    {kv_row("Compactions", &data.session_compactions.to_string())}
                    if let Some(context_usage) = format_context_usage(data.context_tokens, data.context_window) {
                        {kv_row("Context", &context_usage)}
                    }
                    if data.active_delegate_count > 0 {
                        {kv_row(
                            "Active delegates",
                            &data.active_delegate_count.to_string(),
                        )}
                    }
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
struct SessionControlSummaryItem {
    label: &'static str,
    value: String,
    compact: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SessionAlert {
    class_name: &'static str,
    tone: &'static str,
    title: &'static str,
    body: String,
}

fn session_control_summary(data: &SessionData) -> Vec<SessionControlSummaryItem> {
    let mut items = vec![
        SessionControlSummaryItem {
            label: "Thinking",
            value: data.thinking_level.clone(),
            compact: false,
        },
        SessionControlSummaryItem {
            label: "Capability tier",
            value: data.capability_tier.clone(),
            compact: false,
        },
        SessionControlSummaryItem {
            label: "Providers",
            value: format_authenticated_provider_summary(&data.providers),
            compact: true,
        },
        SessionControlSummaryItem {
            label: "Delegates",
            value: format!("{} active", data.active_delegate_count),
            compact: false,
        },
    ];

    if let Some(dispatcher) = &data.dispatcher_binding {
        items.push(SessionControlSummaryItem {
            label: "Dispatcher",
            value: switch_target_label(
                Some(dispatcher.expected_profile.as_str()),
                dispatcher.expected_model.as_deref(),
            ),
            compact: true,
        });
    }

    items
}

fn session_alerts(data: &SessionData) -> Vec<SessionAlert> {
    let mut alerts = Vec::new();

    if let Some(warning) = &data.memory_warning {
        alerts.push(SessionAlert {
            class_name: "session-alert session-alert-warn",
            tone: "warn",
            title: "Memory attention needed",
            body: warning.clone(),
        });
    }

    if !data.memory_available {
        alerts.push(SessionAlert {
            class_name: "session-alert session-alert-danger",
            tone: "danger",
            title: "Project memory offline",
            body: "Long-lived memory tools are unavailable, so architectural context and prior decisions will not persist across sessions.".into(),
        });
    }

    if !data.cleave_available {
        alerts.push(SessionAlert {
            class_name: "session-alert session-alert-danger",
            tone: "danger",
            title: "Cleave unavailable",
            body: "Parallel task decomposition controls are offline; multi-step implementation work must stay on a single branch until the dispatcher recovers.".into(),
        });
    }

    if data.dispatcher_binding.is_none() {
        alerts.push(SessionAlert {
            class_name: "session-alert session-alert-muted",
            tone: "muted",
            title: "Dispatcher binding missing",
            body: "The snapshot does not include a dispatcher control-plane binding, so profile switches cannot be verified from this pane.".into(),
        });
    }

    alerts
}

fn format_authenticated_provider_summary(providers: &[crate::fixtures::ProviderInfo]) -> String {
    let authenticated = providers
        .iter()
        .filter(|provider| provider.authenticated)
        .count();
    if providers.is_empty() {
        "0 / 0 authenticated".into()
    } else {
        format!("{authenticated} / {} authenticated", providers.len())
    }
}

fn format_elapsed_ms(elapsed_ms: u64) -> String {
    let total_seconds = elapsed_ms / 1_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;

    if minutes == 0 {
        format!("{seconds}s")
    } else {
        format!("{minutes}m {seconds:02}s")
    }
}

#[allow(dead_code)]
fn render_dispatcher_switch_state(
    dispatcher: &DispatcherBindingData,
    state: &DispatcherSwitchStateData,
    on_transcript_focus: Option<EventHandler<String>>,
) -> Element {
    let view = dispatcher_switch_view(dispatcher, state);
    let detail = view.detail.clone();

    rsx! {
        section { class: "screen-subsection",
            h3 { class: "screen-section-title", "Switch state" }
            if let Some(handler) = on_transcript_focus {
                button {
                    class: "transcript-focus-link",
                    r#type: "button",
                    onclick: {
                        let target = if let Some(request_id) = state.request_id.as_ref() {
                            format!("dispatcher-switch:{request_id}")
                        } else if let Some(profile) = state.requested_profile.as_ref() {
                            format!("dispatcher-switch:{profile}")
                        } else {
                            "dispatcher-switch:".to_string()
                        };
                        move |_| handler.call(target.clone())
                    },
                    "Focus related transcript events"
                }
            }
            div { class: "dispatcher-switch-card",
                "data-surface": "panel",
                "data-state": dispatcher_switch_badge_state(view.badge_status),
                "data-tone": dispatcher_switch_badge_tone(view.badge_status),
                div { class: "dispatcher-switch-header",
                    span { class: dispatcher_switch_badge_class(view.badge_status), "{view.badge_label}" }
                    span { class: "dispatcher-switch-headline", "{view.headline}" }
                }
                if let Some(detail) = detail {
                    p { class: "dispatcher-switch-detail", "{detail}" }
                }
            }
            div { class: "kv-grid",
                {kv_row("Status", &state.status)}
                {kv_row("Binding", &view.binding_summary)}
                if let Some(request_id) = &state.request_id {
                    {kv_row("Request id", request_id)}
                }
                if let Some(profile) = &state.requested_profile {
                    {kv_row("Requested profile", profile)}
                }
                if let Some(model) = &state.requested_model {
                    {kv_row("Requested model", model)}
                }
                if let Some(failure_code) = &state.failure_code {
                    {kv_row("Failure code", failure_code)}
                }
                if let Some(note) = &state.note {
                    {kv_row("Note", note)}
                }
            }
        }
    }
}

fn session_control_item_tone(label: &str) -> &'static str {
    match label {
        "Thinking" | "Capability tier" => "info",
        "Providers" => "accent",
        "Delegates" => "muted",
        "Dispatcher" => "default",
        _ => "default",
    }
}

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "implementing" | "active" => "badge badge-active",
        "decided" | "done" | "resolved" => "badge badge-done",
        "ready" | "actionable" | "pending" => "badge badge-ready",
        "blocked" | "failed" => "badge badge-blocked",
        "superseded" => "badge badge-neutral",
        _ => "badge badge-neutral",
    }
}

fn status_badge_state(status: &str) -> &'static str {
    match status {
        "implementing" => "implementing",
        "active" => "active",
        "decided" => "decided",
        "done" => "done",
        "resolved" => "resolved",
        "ready" => "ready",
        "actionable" => "actionable",
        "pending" => "pending",
        "blocked" => "blocked",
        "failed" => "failed",
        "superseded" => "superseded",
        _ => "neutral",
    }
}

fn status_badge_tone(status: &str) -> &'static str {
    match status {
        "implementing" | "active" => "info",
        "decided" | "done" | "resolved" => "success",
        "ready" | "actionable" | "pending" => "accent",
        "blocked" | "failed" => "danger",
        "superseded" => "muted",
        _ => "muted",
    }
}

#[allow(dead_code)]
fn dispatcher_switch_badge_class(status: &str) -> &'static str {
    status_badge_class(status)
}

#[allow(dead_code)]
fn dispatcher_switch_badge_state(status: &str) -> &'static str {
    status_badge_state(status)
}

#[allow(dead_code)]
fn dispatcher_switch_badge_tone(status: &str) -> &'static str {
    status_badge_tone(status)
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
struct DispatcherSwitchView {
    badge_status: &'static str,
    badge_label: &'static str,
    headline: String,
    detail: Option<String>,
    binding_summary: String,
}

#[allow(dead_code)]
fn dispatcher_option_disabled(
    dispatcher: &DispatcherBindingData,
    option: &DispatcherOptionData,
) -> bool {
    if dispatcher.expected_profile == option.profile && dispatcher.expected_model == option.model {
        return true;
    }

    dispatcher.switch_state.as_ref().is_some_and(|state| {
        state.status == "pending"
            && state.requested_profile.as_deref() == Some(option.profile.as_str())
            && state.requested_model == option.model
    })
}

#[allow(dead_code)]
fn dispatcher_option_button_class(
    dispatcher: &DispatcherBindingData,
    option: &DispatcherOptionData,
) -> &'static str {
    if dispatcher.expected_profile == option.profile && dispatcher.expected_model == option.model {
        "dispatcher-option-button dispatcher-option-button-active"
    } else if dispatcher.switch_state.as_ref().is_some_and(|state| {
        state.status == "pending"
            && state.requested_profile.as_deref() == Some(option.profile.as_str())
            && state.requested_model == option.model
    }) {
        "dispatcher-option-button dispatcher-option-button-pending"
    } else {
        "dispatcher-option-button"
    }
}

#[allow(dead_code)]
fn dispatcher_option_visual_state(
    dispatcher: &DispatcherBindingData,
    option: &DispatcherOptionData,
) -> &'static str {
    if dispatcher.expected_profile == option.profile && dispatcher.expected_model == option.model {
        "active"
    } else if dispatcher.switch_state.as_ref().is_some_and(|state| {
        state.status == "pending"
            && state.requested_profile.as_deref() == Some(option.profile.as_str())
            && state.requested_model == option.model
    }) {
        "pending"
    } else {
        "idle"
    }
}

#[allow(dead_code)]
fn dispatcher_option_status_text(
    dispatcher: &DispatcherBindingData,
    option: &DispatcherOptionData,
) -> Option<&'static str> {
    if dispatcher.expected_profile == option.profile && dispatcher.expected_model == option.model {
        Some("Active binding")
    } else if dispatcher.switch_state.as_ref().is_some_and(|state| {
        state.status == "pending"
            && state.requested_profile.as_deref() == Some(option.profile.as_str())
            && state.requested_model == option.model
    }) {
        Some("Pending request")
    } else {
        None
    }
}

#[allow(dead_code)]
fn dispatcher_switch_view(
    dispatcher: &DispatcherBindingData,
    state: &DispatcherSwitchStateData,
) -> DispatcherSwitchView {
    let binding_summary = switch_target_label(
        Some(dispatcher.expected_profile.as_str()),
        dispatcher.expected_model.as_deref(),
    );
    let requested_target = switch_target_label(
        state.requested_profile.as_deref(),
        state.requested_model.as_deref(),
    );
    let matches_binding = state.requested_profile.as_deref()
        == Some(dispatcher.expected_profile.as_str())
        && match state.requested_model.as_deref() {
            Some(model) => dispatcher.expected_model.as_deref() == Some(model),
            None => true,
        };

    match state.status.as_str() {
        "pending" => DispatcherSwitchView {
            badge_status: "pending",
            badge_label: "pending",
            headline: format!("Awaiting confirmation for {requested_target}"),
            detail: state
                .request_id
                .as_ref()
                .map(|request_id| format!("Request {request_id} has not been confirmed by the backend yet.")),
            binding_summary,
        },
        "active" if state.request_id.is_some() && matches_binding => DispatcherSwitchView {
            badge_status: "active",
            badge_label: "confirmed",
            headline: format!("Confirmed switch to {requested_target}"),
            detail: state
                .request_id
                .as_ref()
                .map(|request_id| format!("Backend confirmed request {request_id} and updated the active binding.")),
            binding_summary,
        },
        "active" if state.request_id.is_some() && !matches_binding => DispatcherSwitchView {
            badge_status: "active",
            badge_label: "active elsewhere",
            headline: format!("Another request is active: {requested_target}"),
            detail: state.request_id.as_ref().map(|request_id| {
                format!(
                    "Backend reports request {request_id} as active, but the bound dispatcher remains {binding_summary}."
                )
            }),
            binding_summary,
        },
        "active" => DispatcherSwitchView {
            badge_status: "active",
            badge_label: "active",
            headline: format!("Dispatcher bound to {binding_summary}"),
            detail: state.note.clone(),
            binding_summary,
        },
        "failed" => DispatcherSwitchView {
            badge_status: "failed",
            badge_label: "failed",
            headline: format!("Switch failed for {requested_target}"),
            detail: state.failure_code.as_ref().map(|code| format!("Backend reported failure code: {code}")),
            binding_summary,
        },
        "superseded" => DispatcherSwitchView {
            badge_status: "superseded",
            badge_label: "superseded",
            headline: format!("Switch superseded: {requested_target}"),
            detail: state
                .request_id
                .as_ref()
                .map(|request_id| format!("Request {request_id} was replaced before becoming active.")),
            binding_summary,
        },
        _ => DispatcherSwitchView {
            badge_status: "unknown",
            badge_label: "status",
            headline: format!("Dispatcher switch status: {}", state.status),
            detail: state.note.clone(),
            binding_summary,
        },
    }
}

#[allow(dead_code)]
fn switch_target_label(profile: Option<&str>, model: Option<&str>) -> String {
    match (profile, model) {
        (Some(profile), Some(model)) => format!("{profile} · {model}"),
        (Some(profile), None) => profile.to_string(),
        (None, Some(model)) => model.to_string(),
        (None, None) => "unknown target".into(),
    }
}

#[allow(dead_code)]
fn format_context_usage(tokens: Option<u64>, window: Option<u64>) -> Option<String> {
    match (tokens, window) {
        (Some(tokens), Some(window)) => Some(format!("{tokens} / {window} tokens")),
        (Some(tokens), None) => Some(format!("{tokens} tokens")),
        (None, Some(window)) => Some(format!("window {window} tokens")),
        (None, None) => None,
    }
}

#[allow(dead_code)]
fn kv_row(key: &str, value: &str) -> Element {
    rsx! {
        div { class: "kv-row",
            span { class: "kv-key", "{key}" }
            span { class: "kv-value", "{value}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> DispatcherBindingData {
        DispatcherBindingData {
            session_id: "session_01".into(),
            dispatcher_instance_id: "dispatcher_01".into(),
            expected_role: "primary-driver".into(),
            expected_profile: "primary-interactive".into(),
            expected_model: Some("anthropic:claude-sonnet-4-6".into()),
            control_plane_schema: 2,
            token_ref: None,
            observed_base_url: None,
            last_verified_at: None,
            available_options: vec![],
            switch_state: None,
        }
    }

    #[test]
    fn format_context_usage_prefers_tokens_and_window() {
        assert_eq!(
            format_context_usage(Some(2048), Some(8192)).as_deref(),
            Some("2048 / 8192 tokens")
        );
        assert_eq!(
            format_context_usage(Some(2048), None).as_deref(),
            Some("2048 tokens")
        );
        assert_eq!(
            format_context_usage(None, Some(8192)).as_deref(),
            Some("window 8192 tokens")
        );
        assert_eq!(format_context_usage(None, None), None);
    }

    #[test]
    fn dispatcher_switch_view_marks_matching_active_request_as_confirmed() {
        let dispatcher = binding();
        let state = DispatcherSwitchStateData {
            request_id: Some("dispatcher-switch-7".into()),
            requested_profile: Some("primary-interactive".into()),
            requested_model: Some("anthropic:claude-sonnet-4-6".into()),
            status: "active".into(),
            failure_code: None,
            note: Some("Dispatcher switch confirmed by snapshot".into()),
        };

        let view = dispatcher_switch_view(&dispatcher, &state);
        assert_eq!(view.badge_label, "confirmed");
        assert!(view.headline.contains("Confirmed switch"));
        assert!(view.detail.unwrap().contains("dispatcher-switch-7"));
    }

    #[test]
    fn dispatcher_switch_view_marks_different_active_request_as_active_elsewhere() {
        let dispatcher = binding();
        let state = DispatcherSwitchStateData {
            request_id: Some("dispatcher-switch-999".into()),
            requested_profile: Some("supervisor-heavy".into()),
            requested_model: Some("openai:gpt-4.1".into()),
            status: "active".into(),
            failure_code: None,
            note: Some("Different request became active".into()),
        };

        let view = dispatcher_switch_view(&dispatcher, &state);
        assert_eq!(view.badge_label, "active elsewhere");
        assert!(view.headline.contains("Another request is active"));
        assert!(
            view.detail
                .unwrap()
                .contains("bound dispatcher remains primary-interactive")
        );
    }

    #[test]
    fn dispatcher_option_helpers_mark_pending_target() {
        let mut dispatcher = binding();
        dispatcher.switch_state = Some(DispatcherSwitchStateData {
            request_id: Some("dispatcher-switch-2".into()),
            requested_profile: Some("supervisor-heavy".into()),
            requested_model: Some("openai:gpt-4.1".into()),
            status: "pending".into(),
            failure_code: None,
            note: None,
        });
        let option = DispatcherOptionData {
            profile: "supervisor-heavy".into(),
            label: "Supervisor Heavy".into(),
            model: Some("openai:gpt-4.1".into()),
        };

        assert!(dispatcher_option_disabled(&dispatcher, &option));
        assert_eq!(
            dispatcher_option_button_class(&dispatcher, &option),
            "dispatcher-option-button dispatcher-option-button-pending"
        );
        assert_eq!(
            dispatcher_option_visual_state(&dispatcher, &option),
            "pending"
        );
        assert_eq!(status_badge_state("pending"), "pending");
        assert_eq!(status_badge_tone("pending"), "accent");
        assert_eq!(
            dispatcher_option_status_text(&dispatcher, &option),
            Some("Pending request")
        );
    }

    #[test]
    fn session_control_summary_reports_provider_and_dispatcher_state() {
        let data = SessionData {
            thinking_level: "high".into(),
            capability_tier: "gloriana".into(),
            providers: vec![
                crate::fixtures::ProviderInfo {
                    name: "github".into(),
                    authenticated: true,
                    model: None,
                },
                crate::fixtures::ProviderInfo {
                    name: "openai".into(),
                    authenticated: false,
                    model: Some("gpt-4.1".into()),
                },
            ],
            active_delegate_count: 2,
            dispatcher_binding: Some(binding()),
            ..SessionData::default()
        };

        let summary = session_control_summary(&data);
        let labels: Vec<_> = summary
            .iter()
            .map(|item| (item.label, item.value.as_str()))
            .collect();

        assert!(labels.contains(&("Thinking", "high")));
        assert!(labels.contains(&("Capability tier", "gloriana")));
        assert!(labels.contains(&("Providers", "1 / 2 authenticated")));
        assert!(labels.contains(&("Delegates", "2 active")));
        assert!(labels.contains(&(
            "Dispatcher",
            "primary-interactive · anthropic:claude-sonnet-4-6"
        )));
    }

    #[test]
    fn session_control_summary_prefers_runtime_target_over_legacy_instance_inference() {
        let mut dispatcher = binding();
        dispatcher.expected_model = Some("openai:gpt-4.1".into());
        dispatcher.expected_profile = "supervisor-heavy".into();
        dispatcher.dispatcher_instance_id = "omg_primary_01HVDEMO".into();
        let data = SessionData {
            dispatcher_binding: Some(dispatcher),
            ..SessionData::default()
        };

        let summary = session_control_summary(&data);
        assert!(summary.iter().any(|item| {
            item.label == "Dispatcher" && item.value == "supervisor-heavy · openai:gpt-4.1"
        }));
    }

    #[test]
    fn session_alerts_include_missing_control_planes_and_memory_warning() {
        let data = SessionData {
            memory_available: false,
            cleave_available: false,
            memory_warning: Some("Context budget nearly exhausted".into()),
            dispatcher_binding: None,
            ..SessionData::default()
        };

        let alerts = session_alerts(&data);
        let titles: Vec<_> = alerts.iter().map(|alert| alert.title).collect();

        assert!(titles.contains(&"Memory attention needed"));
        assert!(titles.contains(&"Project memory offline"));
        assert!(titles.contains(&"Cleave unavailable"));
        assert!(titles.contains(&"Dispatcher binding missing"));
        assert_eq!(alerts[0].tone, "warn");
        assert_eq!(alerts[1].tone, "danger");
        assert_eq!(alerts[3].tone, "muted");
    }

    #[test]
    fn semantic_helper_mappings_cover_badges_controls_and_cards() {
        assert_eq!(session_control_item_tone("Thinking"), "info");
        assert_eq!(session_control_item_tone("Providers"), "accent");
        assert_eq!(status_badge_state("failed"), "failed");
        assert_eq!(status_badge_tone("failed"), "danger");
        assert_eq!(dispatcher_switch_badge_state("active"), "active");
        assert_eq!(dispatcher_switch_badge_tone("superseded"), "muted");
    }

    #[test]
    fn format_elapsed_ms_uses_operator_friendly_units() {
        assert_eq!(format_elapsed_ms(999), "0s");
        assert_eq!(format_elapsed_ms(12_000), "12s");
        assert_eq!(format_elapsed_ms(125_000), "2m 05s");
    }
}
