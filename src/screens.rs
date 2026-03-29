/// Power-mode Work and Session screens.
///
/// These are composed from `WorkData` and `SessionData` view-models
/// derived from the Omegon snapshot; no additional backend calls needed.
use dioxus::prelude::*;

use crate::fixtures::{SessionData, WorkData};

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
pub fn SessionScreen(data: SessionData) -> Element {
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
