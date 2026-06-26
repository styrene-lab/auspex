use dioxus::prelude::*;

// ============================================================
// Surface data model — contract-shaped mock of the daemon's
// /api/web/surfaces snapshot. Components below render these.
// ============================================================

#[derive(Clone, Copy, PartialEq)]
struct WebSurfaceSnapshot {
    runtime: RuntimeSurface,
    launch: LaunchSurface,
    transcript: &'static [TranscriptEvent],
    objectives: &'static [ObjectiveItem],
    context_spark: SparklineSpec,
    commands: &'static [&'static str],
    composer: ComposerSurface,
}

#[derive(Clone, Copy, PartialEq)]
struct RuntimeSurface {
    agent_id: &'static str,
    state: &'static str,
    workspace: &'static str,
    model: &'static str,
    context_window: &'static str,
    tool_count: u16,
    tool_online: u16,
    tool_sockets: u16,
    transport: &'static str,
    link_status: &'static str,
    latency: &'static str,
    autonomy: &'static str,
    uptime: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
struct LaunchSurface {
    title: &'static str,
    subtitle: &'static str,
    policy_owner: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
struct ComposerSurface {
    queue_mode: &'static str,
    initial_prompt: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
struct TranscriptEvent {
    role: &'static str,
    label: &'static str,
    body: &'static str,
    meta: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
struct ObjectiveItem {
    state_class: &'static str,
    glyph: &'static str,
    label: &'static str,
    status: &'static str,
    progress: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
struct SparklineSpec {
    label: &'static str,
    bars: &'static [&'static str],
}

// ---- Mock data ---------------------------------------------

const CONTEXT_SPARK: SparklineSpec = SparklineSpec {
    label: "Context load",
    bars: &["35%", "42%", "52%", "64%", "58%", "72%", "68%"],
};

const OBJECTIVES: &[ObjectiveItem] = &[
    ObjectiveItem {
        state_class: "complete",
        glyph: "✓",
        label: "surface snapshot contract",
        status: "4/4",
        progress: "100%",
    },
    ObjectiveItem {
        state_class: "active",
        glyph: "›",
        label: "UI action transport",
        status: "2/5",
        progress: "42%",
    },
    ObjectiveItem {
        state_class: "gated",
        glyph: "!",
        label: "approval and tool cards",
        status: "gated",
        progress: "18%",
    },
    ObjectiveItem {
        state_class: "queued",
        glyph: "·",
        label: "Auspex launch context",
        status: "queued",
        progress: "8%",
    },
];

const TRANSCRIPT: &[TranscriptEvent] = &[
    TranscriptEvent {
        role: "operator",
        label: "Operator",
        body: "Pick up the 0.1.0 release candidate work — I want the Omegon Web surface demoable by Friday, running off the daemon snapshot instead of mock data.",
        meta: "prompt · 7m ago",
    },
    TranscriptEvent {
        role: "assistant",
        label: "Omegon",
        body: "Understood. The release framework already verifies, so the critical path is the daemon-owned web surface. I'll land it in three slices: contract-shaped snapshot first, live transport second, action round-trip last.",
        meta: "assistant · 7m ago",
    },
    TranscriptEvent {
        role: "tool",
        label: "Tool call",
        body: "GET /api/web/surfaces → 200. Returned runtime, transcript, objectives, and approval surfaces (4 sections, 11 events).",
        meta: "surface snapshot · 41 ms",
    },
    TranscriptEvent {
        role: "lifecycle",
        label: "Plan advanced",
        body: "Objective 'surface snapshot contract' marked complete. The semantic snapshot now matches the Rust struct one-to-one — no shadow shapes.",
        meta: "plan · objective 1 of 4",
    },
    TranscriptEvent {
        role: "operator",
        label: "Operator",
        body: "Good. Start on the UI action transport so Send and the approval buttons actually reach the daemon.",
        meta: "prompt · 4m ago",
    },
    TranscriptEvent {
        role: "assistant",
        label: "Omegon",
        body: "I'll open a WebSocket to the surfaces stream and post operator actions as intents. The daemon stays authoritative; the surface only proposes. Approval gates resolve server-side and echo back as lifecycle events.",
        meta: "assistant · 4m ago",
    },
    TranscriptEvent {
        role: "tool",
        label: "Tool call",
        body: "cargo test --features omegon-web-mock surfaces:: → 12 passed, 0 failed. Snapshot round-trips through serde without drift.",
        meta: "test run · 1.8 s",
    },
    TranscriptEvent {
        role: "lifecycle",
        label: "Memory written",
        body: "Stored constraint: the web surface must never mutate state locally — every action is a proposal the daemon accepts or rejects.",
        meta: "memory · constraints",
    },
    TranscriptEvent {
        role: "assistant",
        label: "Omegon",
        body: "Transport scaffolding is in place. I'm wiring the context meter and objective rows to live snapshot fields now, then I'll exercise a real approval round-trip.",
        meta: "assistant · streaming",
    },
    TranscriptEvent {
        role: "tool",
        label: "Tool call",
        body: "cargo check --target wasm32-unknown-unknown --no-default-features --features omegon-web-mock",
        meta: "build · running",
    },
    TranscriptEvent {
        role: "approval",
        label: "Approval needed",
        body: "Allow shell command: trunk build web/omegon-web-mock.html and publish the bundle to the daemon's static surface route.",
        meta: "policy gate · pending",
    },
];

const COMMANDS: &[&str] = &[
    "/continue",
    "/clear",
    "/plan status",
    "/tools",
    "/memory status",
    "/model",
    "/approve",
    "/diff",
];

const MOCK_SURFACE: WebSurfaceSnapshot = WebSurfaceSnapshot {
    runtime: RuntimeSurface {
        agent_id: "daemon-01",
        state: "attached",
        workspace: "/Users/wilson/workspace/styrene-labs/auspex",
        model: "openai-codex:gpt-5.5",
        context_window: "82k / 128k",
        tool_count: 17,
        tool_online: 9,
        tool_sockets: 12,
        transport: "ws://daemon/api/web/surfaces/stream",
        link_status: "stream nominal",
        latency: "38 ms",
        autonomy: "conservative",
        uptime: "01:24:36",
    },
    launch: LaunchSurface {
        title: "Persistent agent chat",
        subtitle: "Daemon-owned single-agent surface · opened standalone or through Auspex",
        policy_owner: "local daemon",
    },
    transcript: TRANSCRIPT,
    objectives: OBJECTIVES,
    context_spark: CONTEXT_SPARK,
    commands: COMMANDS,
    composer: ComposerSurface {
        queue_mode: "interruptible",
        initial_prompt: "Once the build clears, run the approval round-trip end to end and report latency.",
    },
};

// ============================================================
// Reusable primitives — the shared HUD vocabulary.
// ============================================================

/// Tick + label + extending-rule section header.
#[component]
fn Eyebrow(label: &'static str) -> Element {
    rsx! { div { class: "owm-eyebrow", "{label}" } }
}

/// Amber signal chip for live status words.
#[component]
fn StateChip(label: &'static str) -> Element {
    rsx! { div { class: "owm-state-chip", "{label}" } }
}

/// Faceted instrument sigil (daemon core).
#[component]
fn Sigil(class: &'static str) -> Element {
    rsx! { div { class: "{class}" } }
}

/// Label + amber value row used above meters.
#[component]
fn MeterHead(label: &'static str, value: String) -> Element {
    rsx! {
        div { class: "owm-meter-head",
            span { "{label}" }
            strong { "{value}" }
        }
    }
}

/// Segmented capacity gauge.
#[component]
fn SegmentMeter() -> Element {
    rsx! { div { class: "owm-segment-meter", aria_label: "capacity" } }
}

/// Slotted socket grid; `online` slots are lit.
#[component]
fn SocketGrid(online: u16, total: u16) -> Element {
    rsx! {
        div { class: "owm-socket-grid",
            for index in 0..total {
                i { class: if index < online { "online" } else { "idle" } }
            }
        }
    }
}

/// Unboxed micro bar-chart.
#[component]
fn Sparkline(spec: SparklineSpec) -> Element {
    rsx! {
        div { class: "owm-sparkline",
            span { "{spec.label}" }
            div { class: "owm-spark-bars",
                for bar in spec.bars.iter() {
                    i { style: "--h: {bar}" }
                }
            }
        }
    }
}

/// One objective row: glyph, label, status, progress strip.
#[component]
fn ObjectiveRow(item: ObjectiveItem) -> Element {
    rsx! {
        div { class: "owm-objective-row {item.state_class}",
            span { "{item.glyph}" }
            strong { "{item.label}" }
            em { "{item.status}" }
            i { style: "--p: {item.progress}" }
        }
    }
}

/// Container for a sidebar instrument: panel + eyebrow + body.
#[component]
fn InstrumentCard(modifier: &'static str, eyebrow: &'static str, children: Element) -> Element {
    rsx! {
        section { class: "owm-panel owm-rail-card {modifier}",
            Eyebrow { label: eyebrow }
            {children}
        }
    }
}

// ============================================================
// Composed surfaces.
// ============================================================

#[component]
fn HudReadout(label: &'static str, value: String, modifier: &'static str) -> Element {
    rsx! {
        div { class: "owm-hud-readout {modifier}",
            span { class: "owm-hud-readout-label", "{label}" }
            strong { class: "owm-hud-readout-value", "{value}" }
        }
    }
}

/// Link gauge — signal bars + latency, the cockpit's connection instrument.
#[component]
fn LinkGauge(status: &'static str, latency: &'static str) -> Element {
    rsx! {
        div { class: "owm-hud-readout owm-link-gauge",
            span { class: "owm-hud-readout-label", "LINK" }
            div { class: "owm-link-gauge-row",
                div { class: "owm-link-bars",
                    i {}
                    i {}
                    i {}
                    i {}
                }
                strong { class: "owm-hud-readout-value", "{latency}" }
            }
            span { class: "owm-link-gauge-state", "{status}" }
        }
    }
}

/// HUD top strip: mark · instrument readouts · global controls.
/// Replaces the former SaaS title/nav header.
#[component]
fn TopBar(
    launch: LaunchSurface,
    runtime: RuntimeSurface,
    status: &'static str,
    on_palette: EventHandler<()>,
    on_settings: EventHandler<()>,
) -> Element {
    rsx! {
        header { class: "omegon-web-topbar",
            div { class: "owm-hud-mark", title: "{launch.title} · {launch.subtitle}",
                div { class: "owm-core-glyph owm-mark-glyph" }
                span { class: "owm-mark-id", "{runtime.agent_id}" }
            }

            div { class: "owm-hud-readouts",
                HudReadout { label: "WORKSPACE", value: runtime.workspace.to_string(), modifier: "owm-readout-path" }
                HudReadout { label: "MODEL", value: runtime.model.to_string(), modifier: "owm-readout-live" }
                LinkGauge { status: runtime.link_status, latency: runtime.latency }
                HudReadout { label: "AUTONOMY", value: runtime.autonomy.to_string(), modifier: "" }
                HudReadout { label: "UPTIME", value: runtime.uptime.to_string(), modifier: "owm-readout-mono" }
            }

            div { class: "owm-hud-controls",
                omegon-arwes-status-pill { class: "owm-status-pill", status, "{status}" }
                button {
                    class: "owm-hud-knob",
                    title: "Command palette",
                    onclick: move |_| on_palette.call(()),
                    "⌘"
                }
                button {
                    class: "owm-hud-knob",
                    title: "Settings",
                    onclick: move |_| on_settings.call(()),
                    "⚙"
                }
            }
        }
    }
}

#[component]
fn DaemonCoreCard(runtime: RuntimeSurface, context_spark: SparklineSpec) -> Element {
    rsx! {
        InstrumentCard { modifier: "owm-daemon-core", eyebrow: "DAEMON CORE",
            div { class: "owm-core-readout",
                Sigil { class: "owm-core-glyph" }
                div {
                    h3 { "{runtime.agent_id}" }
                    StateChip { label: runtime.state }
                }
            }
            div { class: "owm-meter-block",
                MeterHead { label: "Context window", value: runtime.context_window.to_string() }
                SegmentMeter {}
            }
            div { class: "owm-toolbelt",
                MeterHead { label: "Tool sockets", value: format!("{} active · {} tools", runtime.tool_online, runtime.tool_count) }
                SocketGrid { online: runtime.tool_online, total: runtime.tool_sockets }
            }
            div { class: "owm-spark-grid",
                Sparkline { spec: context_spark }
            }
        }
    }
}

#[component]
fn ObjectivesCard(objectives: &'static [ObjectiveItem]) -> Element {
    rsx! {
        InstrumentCard { modifier: "owm-objectives-card", eyebrow: "OBJECTIVES",
            div { class: "owm-objective-stack",
                for item in objectives.iter() {
                    ObjectiveRow { item: *item }
                }
            }
        }
    }
}

#[component]
fn TranscriptEntry(
    event: TranscriptEvent,
    on_deny: EventHandler<()>,
    on_approve: EventHandler<()>,
) -> Element {
    rsx! {
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
                        onclick: move |_| on_deny.call(()),
                        "Deny"
                    }
                    button {
                        class: "owm-primary-button",
                        onclick: move |_| on_approve.call(()),
                        "Approve"
                    }
                }
            }
        }
    }
}

#[component]
fn Composer(
    queue_mode: &'static str,
    composer: Signal<String>,
    sent_count: Signal<u32>,
    on_settings: EventHandler<()>,
) -> Element {
    let mut composer = composer;
    let mut sent_count = sent_count;
    rsx! {
        section { class: "owm-panel owm-composer-panel",
            div { class: "owm-composer-meta",
                span { "queue mode: {queue_mode}" }
                span { "sent: {sent_count}" }
            }
            textarea {
                value: "{composer}",
                oninput: move |event| composer.set(event.value()),
            }
            div { class: "owm-composer-actions",
                button {
                    class: "owm-ghost-button",
                    onclick: move |_| on_settings.call(()),
                    "Settings"
                }
                button { class: "owm-ghost-button", "Attach" }
                button {
                    class: "owm-primary-button",
                    onclick: move |_| {
                        let next = *sent_count.read() + 1;
                        sent_count.set(next);
                    },
                    "Send"
                }
            }
        }
    }
}

#[component]
fn CommandPalette(commands: &'static [&'static str], on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "owm-modal-scrim", onclick: move |_| on_close.call(()),
            section { class: "owm-modal-card", onclick: move |event| event.stop_propagation(),
                div { class: "owm-modal-head",
                    div {
                        Eyebrow { label: "COMMANDS" }
                        h2 { "Command palette" }
                    }
                    button {
                        class: "owm-ghost-button owm-close-button",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }
                for command in commands.iter() {
                    button {
                        class: "owm-command-button",
                        onclick: move |_| on_close.call(()),
                        "{command}"
                    }
                }
            }
        }
    }
}

#[component]
fn SettingsDrawer(
    policy_owner: &'static str,
    transport: &'static str,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "owm-settings-drawer",
            button { class: "owm-ghost-button", onclick: move |_| on_close.call(()), "Close" }
            h2 { "Settings" }
            p { "Policy owner: {policy_owner}. Auspex may proxy this surface but does not own the session state." }
            div { class: "owm-settings-field",
                span { class: "owm-settings-label", "TRANSPORT" }
                code { class: "owm-settings-value", "{transport}" }
            }
        }
    }
}

// ============================================================
// App root — wires state and composes the surfaces.
// ============================================================

#[component]
pub fn OmegonWebMockApp() -> Element {
    let surface = MOCK_SURFACE;
    let composer = use_signal(|| String::from(surface.composer.initial_prompt));
    let mut palette_open = use_signal(|| false);
    let mut settings_open = use_signal(|| false);
    let mut approval_state = use_signal(|| "pending");
    let sent_count = use_signal(|| 0_u32);

    let status = if *approval_state.read() == "pending" {
        "waiting"
    } else {
        "running"
    };

    rsx! {
        div { class: "omegon-web-shell",
            div { class: "omegon-web-bg" }
            div { class: "hud-frame" }

            TopBar {
                launch: surface.launch,
                runtime: surface.runtime,
                status,
                on_palette: move |_| {
                    let is_open = *palette_open.read();
                    palette_open.set(!is_open);
                },
                on_settings: move |_| settings_open.set(true),
            }

            main { class: "owm-cockpit-layout",
                aside { class: "owm-cockpit-rail owm-left-rail",
                    DaemonCoreCard { runtime: surface.runtime, context_spark: surface.context_spark }
                }

                section { class: "owm-conversation-column",
                    section { class: "owm-panel owm-hero-panel",
                        div { class: "owm-panel-heading",
                            div {
                                Eyebrow { label: "CURRENT TURN" }
                                h2 { "Single-agent transcript" }
                            }
                        }
                        div { class: "owm-transcript-list",
                            for event in surface.transcript.iter() {
                                TranscriptEntry {
                                    event: *event,
                                    on_deny: move |_| approval_state.set("denied"),
                                    on_approve: move |_| approval_state.set("approved"),
                                }
                            }
                        }
                    }

                    Composer {
                        queue_mode: surface.composer.queue_mode,
                        composer,
                        sent_count,
                        on_settings: move |_| settings_open.set(true),
                    }
                }

                aside { class: "owm-cockpit-rail owm-right-rail",
                    ObjectivesCard { objectives: surface.objectives }
                }
            }

            if *palette_open.read() {
                CommandPalette {
                    commands: surface.commands,
                    on_close: move |_| palette_open.set(false),
                }
            }

            if *settings_open.read() {
                SettingsDrawer {
                    policy_owner: surface.launch.policy_owner,
                    transport: surface.runtime.transport,
                    on_close: move |_| settings_open.set(false),
                }
            }
        }
    }
}
