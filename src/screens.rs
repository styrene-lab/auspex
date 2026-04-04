/// Power-mode Work and Session screens.
///
/// These are composed from `WorkData` and `SessionData` view-models
/// derived from the Omegon snapshot; no additional backend calls needed.
use dioxus::prelude::*;

use crate::fixtures::{GraphData, SessionData, WorkData};

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
                            div { class: "graph-count-chip",
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
                            p { class: "work-focused-meta",
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

// ── Session screen ────────────────────────────────────────────────────────────

#[component]
pub fn SessionScreen(
    data: SessionData,
    on_dispatcher_switch: Option<EventHandler<(String, Option<String>)>>,
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
                        {kv_row("Session", &dispatcher.session_id)}
                        {kv_row("Instance", &dispatcher.dispatcher_instance_id)}
                        {kv_row("Role", &dispatcher.expected_role)}
                        {kv_row("Profile", &dispatcher.expected_profile)}
                        if let Some(model) = &dispatcher.expected_model {
                            {kv_row("Model", model)}
                        }
                        {kv_row("Schema", &dispatcher.control_plane_schema.to_string())}
                        if let Some(base_url) = &dispatcher.observed_base_url {
                            {kv_row("Endpoint", base_url)}
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
                                            class: "dispatcher-option-button",
                                            r#type: "button",
                                            disabled: dispatcher.expected_profile == option.profile
                                                && dispatcher.expected_model == option.model,
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
                        section { class: "screen-subsection",
                            h3 { class: "screen-section-title", "Switch state" }
                            div { class: "kv-grid",
                                {kv_row("Status", &state.status)}
                                if let Some(profile) = &state.requested_profile {
                                    {kv_row("Requested profile", profile)}
                                }
                                if let Some(model) = &state.requested_model {
                                    {kv_row("Requested model", model)}
                                }
                                if let Some(note) = &state.note {
                                    {kv_row("Note", note)}
                                }
                            }
                        }
                    }
                }
            }

            // Active delegates
            if !data.active_delegates.is_empty() {
                section { class: "screen-section",
                    h2 { class: "screen-section-title", "Active delegates" }
                    div { class: "kv-grid",
                        for delegate in &data.active_delegates {
                            div { class: "kv-row",
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

fn status_badge_class(status: &str) -> &'static str {
    match status {
        "implementing" | "active" => "badge badge-active",
        "decided" | "done" | "resolved" => "badge badge-done",
        "ready" | "actionable" => "badge badge-ready",
        "blocked" => "badge badge-blocked",
        _ => "badge badge-neutral",
    }
}

fn kv_row(key: &str, value: &str) -> Element {
    rsx! {
        div { class: "kv-row",
            span { class: "kv-key", "{key}" }
            span { class: "kv-value", "{value}" }
        }
    }
}
