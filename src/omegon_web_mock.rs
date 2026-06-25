use dioxus::prelude::*;

#[derive(Clone, Copy)]
struct WebSurfaceSnapshot {
    runtime: RuntimeSurface,
    launch: LaunchSurface,
    transcript: &'static [TranscriptEvent],
    workbench: &'static [&'static str],
    memory_note: &'static str,
    commands: &'static [&'static str],
    composer: ComposerSurface,
}

#[derive(Clone, Copy)]
struct RuntimeSurface {
    agent_id: &'static str,
    state: &'static str,
    workspace: &'static str,
    model: &'static str,
    context_window: &'static str,
    tool_count: u16,
    transport: &'static str,
}

#[derive(Clone, Copy)]
struct LaunchSurface {
    title: &'static str,
    subtitle: &'static str,
    policy_owner: &'static str,
}

#[derive(Clone, Copy)]
struct ComposerSurface {
    queue_mode: &'static str,
    initial_prompt: &'static str,
}

#[derive(Clone, Copy)]
struct TranscriptEvent {
    role: &'static str,
    label: &'static str,
    body: &'static str,
    meta: &'static str,
}

const TRANSCRIPT: &[TranscriptEvent] = &[
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
        role: "lifecycle",
        label: "Workbench",
        body: "Active plan has a bounded mockup slice: contract-shaped fake data first, live daemon transport later.",
        meta: "plan · in progress",
    },
    TranscriptEvent {
        role: "approval",
        label: "Approval needed",
        body: "Allow shell command: cargo check --target wasm32-unknown-unknown --no-default-features --features omegon-web-mock",
        meta: "policy gate · pending",
    },
];

const WORKBENCH_ITEMS: &[&str] = &[
    "surface snapshot contract",
    "UI action transport",
    "approval and tool cards",
    "Auspex launch context",
];

const COMMANDS: &[&str] = &["/continue", "/clear", "/plan status", "/tools", "/memory status"];

const MOCK_SURFACE: WebSurfaceSnapshot = WebSurfaceSnapshot {
    runtime: RuntimeSurface {
        agent_id: "daemon-01",
        state: "attached",
        workspace: "/Users/wilson/workspace/styrene-labs/auspex",
        model: "openai-codex:gpt-5.5",
        context_window: "82k / 128k",
        tool_count: 17,
        transport: "ws://daemon/api/web/surfaces/stream",
    },
    launch: LaunchSurface {
        title: "Persistent agent chat",
        subtitle: "Daemon-owned single-agent surface · opened standalone or through Auspex",
        policy_owner: "local daemon",
    },
    transcript: TRANSCRIPT,
    workbench: WORKBENCH_ITEMS,
    memory_note: "The web app renders semantic Omegon surfaces instead of porting terminal widgets.",
    commands: COMMANDS,
    composer: ComposerSurface {
        queue_mode: "interruptible",
        initial_prompt: "Continue from the release candidate plan.",
    },
};

#[component]
pub fn OmegonWebMockApp() -> Element {
    let surface = MOCK_SURFACE;
    let mut composer = use_signal(|| String::from(surface.composer.initial_prompt));
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
                    omegon-arwes-text { h1 { "{surface.launch.title}" } }
                    p { "{surface.launch.subtitle}" }
                }
                div { class: "status-strip",
                    omegon-arwes-status-pill { class: "status-pill", status: status, "{status}" }
                    span { "workspace {surface.runtime.workspace}" }
                    span { "model {surface.runtime.model}" }
                    span { "transport {surface.runtime.transport}" }
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
                            for event in surface.transcript {
                                article { class: "transcript-card {event.role}",
                                    div { class: "event-head",
                                        strong { "{event.label}" }
                                        span { "{event.meta}" }
                                    }
                                    p { "{event.body}" }
                                    if event.role == "approval" {
                                        div { class: "approval-actions",
                                            omegon-arwes-button {
                                                button {
                                                    class: "danger-button",
                                                    onclick: move |_| approval_state.set("denied"),
                                                    "Deny"
                                                }
                                            }
                                            omegon-arwes-button {
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
                    }

                    omegon-arwes-panel { class: "panel composer-panel", variant: "composer",
                        div { class: "composer-meta",
                            span { "queue mode: {surface.composer.queue_mode}" }
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
                            omegon-arwes-button {
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
                }

                aside { class: "context-rail",
                    omegon-arwes-panel { class: "panel rail-card", variant: "runtime",
                        div { class: "eyebrow", "RUNTIME" }
                        h3 { "{surface.runtime.agent_id}" }
                        dl {
                            div { dt { "State" } dd { "{surface.runtime.state}" } }
                            div { dt { "Context" } dd { "{surface.runtime.context_window}" } }
                            div { dt { "Tools" } dd { "{surface.runtime.tool_count} available" } }
                        }
                    }
                    omegon-arwes-panel { class: "panel rail-card", variant: "workbench",
                        div { class: "eyebrow", "WORKBENCH" }
                        h3 { "Omegon Web mock" }
                        ul {
                            for item in surface.workbench {
                                li { "{item}" }
                            }
                        }
                    }
                    omegon-arwes-panel { class: "panel rail-card", variant: "memory",
                        div { class: "eyebrow", "MEMORY / CONTEXT" }
                        p { "{surface.memory_note}" }
                    }
                }
            }

            if *palette_open.read() {
                div { class: "modal-scrim", onclick: move |_| palette_open.set(false),
                    omegon-arwes-panel { class: "modal-card", variant: "modal", onclick: move |event| event.stop_propagation(),
                        div { class: "modal-head",
                            div {
                                div { class: "eyebrow", "COMMANDS" }
                                h2 { "Command palette" }
                            }
                            button {
                                class: "ghost-button close-button",
                                onclick: move |_| palette_open.set(false),
                                "Close"
                            }
                        }
                        for command in surface.commands {
                            button {
                                class: "command-button",
                                onclick: move |_| palette_open.set(false),
                                "{command}"
                            }
                        }
                    }
                }
            }

            if *settings_open.read() {
                div { class: "settings-drawer",
                    button { class: "ghost-button", onclick: move |_| settings_open.set(false), "Close" }
                    h2 { "Settings" }
                    p { "Policy owner: {surface.launch.policy_owner}. Auspex may proxy this surface but does not own the session state." }
                }
            }
        }
    }
}
