use dioxus::prelude::*;

#[derive(Clone, Copy)]
struct WebSurfaceSnapshot {
    runtime: RuntimeSurface,
    launch: LaunchSurface,
    transcript: &'static [TranscriptEvent],
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
        div { class: "omegon-web-shell",
            div { class: "omegon-web-bg" }
            div { class: "hud-frame" }
            header { class: "omegon-web-topbar",
                div { class: "owm-brand-block",
                    div { class: "owm-eyebrow", "OMEGON WEB" }
                    h1 { "{surface.launch.title}" }
                    p { "{surface.launch.subtitle}" }
                }
                div { class: "owm-status-strip",
                    omegon-arwes-status-pill { class: "owm-status-pill", status: status, "{status}" }
                    span { "workspace {surface.runtime.workspace}" }
                    span { "model {surface.runtime.model}" }
                    span { "transport {surface.runtime.transport}" }
                }
            }

            main { class: "owm-cockpit-layout",
                aside { class: "owm-cockpit-rail owm-left-rail",
                    section { class: "owm-panel owm-rail-card owm-daemon-core",
                        div { class: "owm-eyebrow", "DAEMON CORE" }
                        div { class: "owm-core-readout",
                            div { class: "owm-core-glyph", aria_label: "daemon core" }
                            div {
                                h3 { "{surface.runtime.agent_id}" }
                                div { class: "owm-state-chip", "{surface.runtime.state}" }
                            }
                        }
                        div { class: "owm-meter-block",
                            div { class: "owm-meter-head",
                                span { "Context window" }
                                strong { "{surface.runtime.context_window}" }
                            }
                            div { class: "owm-segment-meter", aria_label: "context capacity" }
                        }
                        div { class: "owm-toolbelt",
                            div { class: "owm-meter-head",
                                span { "Tool sockets" }
                                strong { "{surface.runtime.tool_count} online" }
                            }
                            div { class: "owm-socket-grid",
                                for index in 0..12 {
                                    i { class: if index < 8 { "online" } else { "idle" } }
                                }
                            }
                        }
                        div { class: "owm-spark-grid",
                            div { class: "owm-sparkline",
                                span { "Context load" }
                                div { class: "owm-spark-bars",
                                    i { style: "--h: 35%" }
                                    i { style: "--h: 42%" }
                                    i { style: "--h: 52%" }
                                    i { style: "--h: 64%" }
                                    i { style: "--h: 58%" }
                                    i { style: "--h: 72%" }
                                    i { style: "--h: 68%" }
                                }
                            }
                        }
                    }
                    section { class: "owm-panel owm-rail-card owm-link-card",
                        div { class: "owm-eyebrow", "LINK" }
                        div { class: "owm-link-conduit",
                            span {}
                            span {}
                            span {}
                        }
                        p { "{surface.runtime.transport}" }
                        div { class: "owm-state-chip", "stream nominal" }
                    }
                }

                section { class: "owm-conversation-column",
                    section { class: "owm-panel owm-hero-panel",
                        div { class: "owm-panel-heading",
                            div {
                                div { class: "owm-eyebrow", "CURRENT TURN" }
                                h2 { "Single-agent transcript" }
                            }
                            button {
                                class: "owm-ghost-button",
                                onclick: move |_| {
                                    let is_open = *palette_open.read();
                                    palette_open.set(!is_open);
                                },
                                "Command palette"
                            }
                        }
                        div { class: "owm-transcript-list",
                            for event in surface.transcript {
                                article { class: "owm-transcript-card {event.role}",
                                    div { class: "owm-event-head",
                                        strong { "{event.label}" }
                                        span { "{event.meta}" }
                                    }
                                    p { "{event.body}" }
                                    if event.role == "approval" {
                                        div { class: "owm-approval-actions",
                                            button {
                                                    class: "owm-danger-button",
                                                    onclick: move |_| approval_state.set("denied"),
                                                    "Deny"
                                                }
                                            button {
                                                    class: "owm-primary-button",
                                                    onclick: move |_| approval_state.set("approved"),
                                                    "Approve"
                                                }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    section { class: "owm-panel owm-composer-panel",
                        div { class: "owm-composer-meta",
                            span { "queue mode: {surface.composer.queue_mode}" }
                            span { "sent: {sent_count}" }
                        }
                        textarea {
                            value: "{composer}",
                            oninput: move |event| composer.set(event.value()),
                        }
                        div { class: "owm-composer-actions",
                            button {
                                class: "owm-ghost-button",
                                onclick: move |_| settings_open.set(true),
                                "Settings"
                            }
                            button { class: "owm-ghost-button", "Attach" }
                            button {
                                    class: "owm-primary-button",
                                    onclick: move |_| {
                                        let next_count = *sent_count.read() + 1;
                                        sent_count.set(next_count);
                                    },
                                    "Send"
                                }
                        }
                    }
                }

                aside { class: "owm-cockpit-rail owm-right-rail",
                    section { class: "owm-panel owm-rail-card owm-objectives-card",
                        div { class: "owm-eyebrow", "OBJECTIVES" }
                        div { class: "owm-objective-stack",
                            div { class: "owm-objective-row complete",
                                span { "✓" }
                                strong { "surface snapshot contract" }
                                em { "4/4" }
                                i { style: "--p: 100%" }
                            }
                            div { class: "owm-objective-row active",
                                span { "›" }
                                strong { "UI action transport" }
                                em { "2/5" }
                                i { style: "--p: 42%" }
                            }
                            div { class: "owm-objective-row gated",
                                span { "!" }
                                strong { "approval and tool cards" }
                                em { "gated" }
                                i { style: "--p: 18%" }
                            }
                            div { class: "owm-objective-row queued",
                                span { "·" }
                                strong { "Auspex launch context" }
                                em { "queued" }
                                i { style: "--p: 8%" }
                            }
                        }
                    }
                    section { class: "owm-panel owm-rail-card owm-codex-card",
                        div { class: "owm-eyebrow", "CODEX" }
                        div { class: "owm-codex-core",
                            div { class: "owm-codex-glyph" }
                            div {
                                h3 { "semantic surfaces" }
                                p { "{surface.memory_note}" }
                            }
                        }
                        div { class: "owm-archive-meter" }
                    }
                }
            }

            if *palette_open.read() {
                div { class: "owm-modal-scrim", onclick: move |_| palette_open.set(false),
                    section { class: "owm-modal-card", onclick: move |event| event.stop_propagation(),
                        div { class: "owm-modal-head",
                            div {
                                div { class: "owm-eyebrow", "COMMANDS" }
                                h2 { "Command palette" }
                            }
                            button {
                                class: "owm-ghost-button owm-close-button",
                                onclick: move |_| palette_open.set(false),
                                "Close"
                            }
                        }
                        for command in surface.commands {
                            button {
                                class: "owm-command-button",
                                onclick: move |_| palette_open.set(false),
                                "{command}"
                            }
                        }
                    }
                }
            }

            if *settings_open.read() {
                div { class: "owm-settings-drawer",
                    button { class: "owm-ghost-button", onclick: move |_| settings_open.set(false), "Close" }
                    h2 { "Settings" }
                    p { "Policy owner: {surface.launch.policy_owner}. Auspex may proxy this surface but does not own the session state." }
                }
            }
        }
    }
}
