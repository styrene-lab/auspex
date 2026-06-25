use dioxus::prelude::*;

#[derive(Clone, Copy)]
struct TranscriptEvent {
    role: &'static str,
    label: &'static str,
    body: &'static str,
    meta: &'static str,
}

const EVENTS: &[TranscriptEvent] = &[
    TranscriptEvent {
        role: "operator",
        label: "Operator",
        body: "Sketch the next Auspex release path and keep the web chat surface moving.",
        meta: "prompt · queued",
    },
    TranscriptEvent {
        role: "assistant",
        label: "Omegon",
        body: "Release framework is verified. Next concrete slice is the daemon-owned Omegon Web SPA, opened directly or through Auspex.",
        meta: "assistant · streaming",
    },
    TranscriptEvent {
        role: "tool",
        label: "Tool call",
        body: "GET /api/web/surfaces returned transcript, workbench, runtime, memory, tools, and approval surfaces.",
        meta: "surface snapshot · 42 ms",
    },
    TranscriptEvent {
        role: "approval",
        label: "Approval needed",
        body: "Allow shell command: cargo check --target wasm32-unknown-unknown --no-default-features --features omegon-web-mock",
        meta: "policy gate · pending",
    },
];

#[component]
pub fn OmegonWebMockApp() -> Element {
    let mut composer = use_signal(|| String::from("Continue from the release candidate plan."));
    let mut palette_open = use_signal(|| false);
    let mut settings_open = use_signal(|| false);
    let mut approval_state = use_signal(|| "pending");
    let mut sent_count = use_signal(|| 0_u32);

    let status = if *approval_state.read() == "pending" {
        "waiting"
    } else {
        "running"
    };

    rsx! {
        omegon-arwes-app-shell { class: "omegon-web-shell",
            omegon-arwes-bg { class: "omegon-web-bg" }
            header { class: "omegon-web-topbar",
                div { class: "brand-block",
                    div { class: "eyebrow", "OMEGON WEB" }
                    h1 { "Persistent agent chat" }
                    p { "Daemon-owned single-agent surface · opened standalone or through Auspex" }
                }
                div { class: "status-strip",
                    omegon-arwes-status-pill { class: "status-pill", status: status, "{status}" }
                    span { "workspace /Users/wilson/workspace/styrene-labs/auspex" }
                    span { "model openai-codex:gpt-5.5" }
                    span { "transport ws://daemon/api/web/surfaces/stream" }
                }
            }

            main { class: "omegon-web-layout",
                section { class: "conversation-column",
                    omegon-arwes-panel { class: "panel hero-panel", variant: "primary",
                        div { class: "panel-heading",
                            div {
                                div { class: "eyebrow", "CURRENT TURN" }
                                h2 { "Single-agent transcript" }
                            }
                            button {
                                class: "ghost-button",
                                onclick: move |_| {
                                    let is_open = *palette_open.read();
                                    palette_open.set(!is_open);
                                },
                                "Command palette"
                            }
                        }
                        div { class: "transcript-list",
                            for event in EVENTS {
                                article { class: "transcript-card {event.role}",
                                    div { class: "event-head",
                                        strong { "{event.label}" }
                                        span { "{event.meta}" }
                                    }
                                    p { "{event.body}" }
                                    if event.role == "approval" {
                                        div { class: "approval-actions",
                                            button {
                                                class: "danger-button",
                                                onclick: move |_| approval_state.set("denied"),
                                                "Deny"
                                            }
                                            button {
                                                class: "primary-button",
                                                onclick: move |_| approval_state.set("approved"),
                                                "Approve"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    omegon-arwes-panel { class: "panel composer-panel", variant: "composer",
                        div { class: "composer-meta",
                            span { "queue mode: interruptible" }
                            span { "sent: {sent_count}" }
                        }
                        textarea {
                            value: "{composer}",
                            oninput: move |event| composer.set(event.value()),
                        }
                        div { class: "composer-actions",
                            button {
                                class: "ghost-button",
                                onclick: move |_| settings_open.set(true),
                                "Settings"
                            }
                            button { class: "ghost-button", "Attach" }
                            button {
                                class: "primary-button",
                                onclick: move |_| {
                                    let next_count = *sent_count.read() + 1;
                                    sent_count.set(next_count);
                                },
                                "Send"
                            }
                        }
                    }
                }

                aside { class: "context-rail",
                    omegon-arwes-panel { class: "panel rail-card", variant: "runtime",
                        div { class: "eyebrow", "RUNTIME" }
                        h3 { "daemon-01" }
                        dl {
                            div { dt { "State" } dd { "attached" } }
                            div { dt { "Context" } dd { "82k / 128k" } }
                            div { dt { "Tools" } dd { "17 available" } }
                        }
                    }
                    omegon-arwes-panel { class: "panel rail-card", variant: "workbench",
                        div { class: "eyebrow", "WORKBENCH" }
                        h3 { "Omegon Web mock" }
                        ul {
                            li { "surface snapshot contract" }
                            li { "UI action transport" }
                            li { "approval and tool cards" }
                            li { "Auspex launch context" }
                        }
                    }
                    omegon-arwes-panel { class: "panel rail-card", variant: "memory",
                        div { class: "eyebrow", "MEMORY / CONTEXT" }
                        p { "The web app renders semantic Omegon surfaces instead of porting terminal widgets." }
                    }
                }
            }

            if *palette_open.read() {
                div { class: "modal-scrim", onclick: move |_| palette_open.set(false),
                    omegon-arwes-panel { class: "modal-card", variant: "modal", onclick: move |event| event.stop_propagation(),
                        div { class: "eyebrow", "COMMANDS" }
                        h2 { "Command palette" }
                        button { "/continue" }
                        button { "/clear" }
                        button { "/plan status" }
                    }
                }
            }

            if *settings_open.read() {
                div { class: "settings-drawer",
                    button { class: "ghost-button", onclick: move |_| settings_open.set(false), "Close" }
                    h2 { "Settings" }
                    p { "Policy owner: local daemon. Auspex may proxy this surface but does not own the session state." }
                }
            }
        }
    }
}
