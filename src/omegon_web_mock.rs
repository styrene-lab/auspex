use dioxus::prelude::*;

// ============================================================
// Surface data model — contract-shaped mock of the daemon's
// /api/web/surfaces snapshot. Components below render these.
// ============================================================

#[derive(Clone, Copy, PartialEq)]
enum ModalTarget {
    Settings,
    Config,
    Armory,
    Commands,
}

#[derive(Clone, Copy, PartialEq)]
struct MenuSurface {
    items: &'static [MenuItem],
}

#[derive(Clone, Copy, PartialEq)]
struct MenuItem {
    target: ModalTarget,
    eyebrow: &'static str,
    label: &'static str,
    summary: &'static str,
    signal: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
struct ModalSection {
    label: &'static str,
    value: &'static str,
    detail: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
struct ModalSurface {
    target: ModalTarget,
    eyebrow: &'static str,
    title: &'static str,
    summary: &'static str,
    sections: &'static [ModalSection],
}

#[derive(Clone, Copy, PartialEq)]
struct SessionLinks {
    surfaces: Option<&'static str>,
    actions: Option<&'static str>,
    stream: Option<&'static str>,
}

#[derive(Clone, Copy, PartialEq)]
struct SessionDescriptor {
    schema_version: u8,
    session_id: &'static str,
    current: bool,
    assistant_profile_id: Option<&'static str>,
    assistant_readiness: Option<&'static str>,
    links: SessionLinks,
}

struct WebSurfaceSnapshot {
    session: SessionDescriptor,
    runtime: RuntimeSurface,
    launch: LaunchSurface,
    transcript: &'static [TranscriptEvent],
    plan: PlanLane,
    operations: OperationSurface,
    daemon_events: DaemonEventsSurface,
    context_spark: SparklineSpec,
    menu: MenuSurface,
    modal_surfaces: &'static [ModalSurface],
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
    /// Present on tool calls: full payload shown in the expansion modal.
    detail: &'static str,
}

/// Mirrors `omegon_traits::PlanItemProjection`: status is one of
/// pending|active|done|skipped; intent tags the kind of work.
#[derive(Clone, Copy, PartialEq)]
struct PlanItem {
    status: &'static str,
    intent: &'static str,
    label: &'static str,
    progress: &'static str,
}

/// Mirrors `omegon_traits::PlanLaneProjection`: a mode + progress + items.
#[derive(Clone, Copy, PartialEq)]
struct PlanLane {
    mode: &'static str,
    completed: usize,
    total: usize,
    items: &'static [PlanItem],
}

/// Mirrors `omegon_traits::OperationChildProjection`.
#[derive(Clone, Copy, PartialEq)]
struct OperationChild {
    label: &'static str,
    status: &'static str,
    activity: &'static str,
    progress: &'static str,
    progress_pct: &'static str,
}

/// Mirrors `omegon_traits::OperationProjection`: delegate / cleave / background.
#[derive(Clone, Copy, PartialEq)]
struct OperationSurface {
    kind: &'static str,
    running: usize,
    completed: usize,
    failed: usize,
    children: &'static [OperationChild],
}

/// Mirrors `/api/events/stream` daemon/app SSE payloads.
#[derive(Clone, Copy, PartialEq)]
struct DaemonEventItem {
    event_type: &'static str,
    lane: &'static str,
    summary: &'static str,
    age: &'static str,
}

/// Mirrors the daemon event snapshot/stream pair: `/api/events` + `/api/events/stream`.
#[derive(Clone, Copy, PartialEq)]
struct DaemonEventsSurface {
    queued: usize,
    processed: usize,
    stream_href: &'static str,
    snapshot_href: &'static str,
    events: &'static [DaemonEventItem],
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

const PLAN: PlanLane = PlanLane {
    mode: "executing",
    completed: 1,
    total: 4,
    items: &[
        PlanItem {
            status: "done",
            intent: "spec",
            label: "surface snapshot contract",
            progress: "100%",
        },
        PlanItem {
            status: "active",
            intent: "implementation",
            label: "UI action transport",
            progress: "42%",
        },
        PlanItem {
            status: "pending",
            intent: "review",
            label: "approval and tool cards",
            progress: "0%",
        },
        PlanItem {
            status: "pending",
            intent: "operations",
            label: "Auspex launch context",
            progress: "0%",
        },
    ],
};

const OPERATIONS: OperationSurface = OperationSurface {
    kind: "cleave",
    running: 2,
    completed: 1,
    failed: 0,
    children: &[
        OperationChild {
            label: "transport-wiring",
            status: "running",
            activity: "editing action_bridge.rs",
            progress: "3/5",
            progress_pct: "60%",
        },
        OperationChild {
            label: "approval-cards",
            status: "running",
            activity: "rendering policy gate",
            progress: "1/3",
            progress_pct: "33%",
        },
        OperationChild {
            label: "snapshot-scout",
            status: "done",
            activity: "returned 4 sections",
            progress: "2/2",
            progress_pct: "100%",
        },
    ],
};

const DAEMON_EVENTS: DaemonEventsSurface = DaemonEventsSurface {
    queued: 2,
    processed: 47,
    stream_href: "/api/events/stream",
    snapshot_href: "/api/events",
    events: &[
        DaemonEventItem {
            event_type: "runtime.context_changed",
            lane: "runtime",
            summary: "context window recalculated: 82k / 128k tokens",
            age: "12s",
        },
        DaemonEventItem {
            event_type: "lifecycle.snapshot_changed",
            lane: "plan",
            summary: "plan projection updated; active lane still executing",
            age: "33s",
        },
        DaemonEventItem {
            event_type: "provider.status_changed",
            lane: "provider",
            summary: "openai-codex:gpt-5.5 remains selected and serving",
            age: "1m",
        },
        DaemonEventItem {
            event_type: "stream.lagged",
            lane: "recovery",
            summary: "client skipped events; refetch snapshot from /api/events",
            age: "3m",
        },
    ],
};

const TRANSCRIPT: &[TranscriptEvent] = &[
    TranscriptEvent {
        role: "operator",
        label: "Operator",
        body: "Pick up the 0.1.0 release candidate work — I want the Omegon Web surface demoable by Friday, running off the daemon snapshot instead of mock data.",
        meta: "prompt · 7m ago",
        detail: "",
    },
    TranscriptEvent {
        role: "assistant",
        label: "Omegon",
        body: "Understood. The release framework already verifies, so the critical path is the daemon-owned web surface. I'll land it in three slices: contract-shaped snapshot first, live transport second, action round-trip last.",
        meta: "assistant · 7m ago",
        detail: "",
    },
    TranscriptEvent {
        role: "tool",
        label: "Tool call",
        body: "GET /api/web/surfaces → 200. Returned runtime, transcript, objectives, and approval surfaces (4 sections, 11 events).",
        meta: "surface snapshot · 41 ms",
        detail: "GET /api/web/surfaces\nstatus 200 · 41 ms\n\nsections: runtime, transcript, objectives, approval\nevents: 11\nbytes: 4.2 KB",
    },
    TranscriptEvent {
        role: "lifecycle",
        label: "Plan advanced",
        body: "Objective 'surface snapshot contract' marked complete. The semantic snapshot now matches the Rust struct one-to-one — no shadow shapes.",
        meta: "plan · objective 1 of 4",
        detail: "",
    },
    TranscriptEvent {
        role: "operator",
        label: "Operator",
        body: "Good. Start on the UI action transport so Send and the approval buttons actually reach the daemon.",
        meta: "prompt · 4m ago",
        detail: "",
    },
    TranscriptEvent {
        role: "assistant",
        label: "Omegon",
        body: "I'll open a WebSocket to the surfaces stream and post operator actions as intents. The daemon stays authoritative; the surface only proposes. Approval gates resolve server-side and echo back as lifecycle events.",
        meta: "assistant · 4m ago",
        detail: "",
    },
    TranscriptEvent {
        role: "tool",
        label: "Tool call",
        body: "cargo test --features omegon-web-mock surfaces:: → 12 passed, 0 failed. Snapshot round-trips through serde without drift.",
        meta: "test run · 1.8 s",
        detail: "cargo test --features omegon-web-mock\n\nrunning 12 tests\n............ ok\n\ntest result: ok. 12 passed; 0 failed; 0 ignored\nfinished in 1.81s",
    },
    TranscriptEvent {
        role: "lifecycle",
        label: "Memory written",
        body: "Stored constraint: the web surface must never mutate state locally — every action is a proposal the daemon accepts or rejects.",
        meta: "memory · constraints",
        detail: "",
    },
    TranscriptEvent {
        role: "assistant",
        label: "Omegon",
        body: "Transport scaffolding is in place. I'm wiring the context meter and objective rows to live snapshot fields now, then I'll exercise a real approval round-trip.",
        meta: "assistant · streaming",
        detail: "",
    },
    TranscriptEvent {
        role: "tool",
        label: "Tool call",
        body: "cargo check --target wasm32-unknown-unknown --no-default-features --features omegon-web-mock",
        meta: "build · running",
        detail: "cargo build --target wasm32-unknown-unknown\n  --no-default-features --features omegon-web-mock\n\nCompiling auspex v0.2.0-rc.1\n  Building [=========>        ] 218/256\nstatus: running",
    },
    TranscriptEvent {
        role: "approval",
        label: "Approval needed",
        body: "Allow shell command: trunk build web/omegon-web-mock.html and publish the bundle to the daemon's static surface route.",
        meta: "policy gate · pending",
        detail: "",
    },
];


const MENU_ITEMS: &[MenuItem] = &[
    MenuItem {
        target: ModalTarget::Settings,
        eyebrow: "SESSION",
        label: "Settings",
        summary: "Policy owner, transport, and local authority profile.",
        signal: "nominal",
    },
    MenuItem {
        target: ModalTarget::Config,
        eyebrow: "SURFACE",
        label: "Config",
        summary: "Renderer bindings, data source selection, and stream behavior.",
        signal: "draft",
    },
    MenuItem {
        target: ModalTarget::Armory,
        eyebrow: "TOOLS",
        label: "Armory",
        summary: "Enabled capabilities, safety posture, and tool inventory.",
        signal: "17 tools",
    },
    MenuItem {
        target: ModalTarget::Commands,
        eyebrow: "INPUT",
        label: "Commands",
        summary: "Slash commands and quick operator intents.",
        signal: "8 ready",
    },
];

const MENU_SURFACE: MenuSurface = MenuSurface { items: MENU_ITEMS };

const SETTINGS_SECTIONS: &[ModalSection] = &[
    ModalSection { label: "Session", value: "default · current", detail: "Native session envelope from POST /api/sessions; singleton for phase 1." },
    ModalSection { label: "Proxy", value: "auspex · trusted", detail: "Auspex forwards standard Omegon-Principal-* headers plus Omegon-Back-Url." },
    ModalSection { label: "Policy owner", value: "auspex", detail: "RBAC decisions are evaluated by Omegon using the trusted Auspex principal." },
    ModalSection { label: "Transport", value: "/api/sessions/default/surfaces/stream", detail: "Native session stream; legacy /api/web/surfaces remains a compatibility path." },
    ModalSection { label: "Autonomy", value: "conservative", detail: "Operator confirmations required for elevated or ambiguous actions." },
];

const CONFIG_SECTIONS: &[ModalSection] = &[
    ModalSection { label: "Bootstrap", value: "POST /api/sessions", detail: "Create or attach first, then follow daemon-provided links instead of hardcoded web paths." },
    ModalSection { label: "Snapshot", value: "/api/sessions/default/surfaces", detail: "HTTP bootstrap for the current semantic surface bundle." },
    ModalSection { label: "Actions", value: "/api/sessions/default/actions", detail: "Prompt sends, approvals, and UI actions post here; daemon remains authoritative." },
    ModalSection { label: "Transcript mode", value: "chat + collapsed tools", detail: "Operator/agent prose stays in the feed; tool payloads expand in modal overlays." },
];

const ARMORY_SECTIONS: &[ModalSection] = &[
    ModalSection { label: "Shell", value: "enabled · gated", detail: "Command execution is available through policy prompts and audit entries." },
    ModalSection { label: "Files", value: "read/write scoped", detail: "Workspace edits remain constrained to approved project roots." },
    ModalSection { label: "Subagents", value: "delegate · cleave", detail: "Background workers surface as Operations rows instead of transcript noise." },
];

const COMMAND_SECTIONS: &[ModalSection] = &[
    ModalSection { label: "/continue", value: "resume plan", detail: "Continue from the active plan item." },
    ModalSection { label: "/plan status", value: "inspect", detail: "Open the current plan lane and workstream summary." },
    ModalSection { label: "/tools", value: "armory", detail: "Inspect available tool surfaces and safety gates." },
];

const MODAL_SURFACES: &[ModalSurface] = &[
    ModalSurface { target: ModalTarget::Settings, eyebrow: "SETTINGS", title: "Session settings", summary: "Session authority and transport controls. These are global levers, not chat content.", sections: SETTINGS_SECTIONS },
    ModalSurface { target: ModalTarget::Config, eyebrow: "CONFIG", title: "Surface configuration", summary: "Renderer and data-source settings for the web cockpit.", sections: CONFIG_SECTIONS },
    ModalSurface { target: ModalTarget::Armory, eyebrow: "ARMORY", title: "Capability armory", summary: "Curated tool/capability inventory with safety posture attached.", sections: ARMORY_SECTIONS },
    ModalSurface { target: ModalTarget::Commands, eyebrow: "COMMANDS", title: "Command palette", summary: "Operator shortcuts and command-shaped intents.", sections: COMMAND_SECTIONS },
];

const MOCK_SURFACE: WebSurfaceSnapshot = WebSurfaceSnapshot {
    session: SessionDescriptor {
        schema_version: 1,
        session_id: "default",
        current: true,
        assistant_profile_id: Some("omegon-default"),
        assistant_readiness: Some("ready"),
        links: SessionLinks {
            surfaces: Some("/api/sessions/default/surfaces"),
            actions: Some("/api/sessions/default/actions"),
            stream: Some("/api/sessions/default/surfaces/stream"),
        },
    },
    runtime: RuntimeSurface {
        agent_id: "daemon-01",
        state: "attached",
        workspace: "/Users/wilson/workspace/styrene-labs/auspex",
        model: "openai-codex:gpt-5.5",
        context_window: "82k / 128k",
        tool_count: 17,
        tool_online: 9,
        tool_sockets: 12,
        transport: "/api/sessions/default/surfaces/stream",
        link_status: "stream nominal",
        latency: "38 ms",
        autonomy: "conservative",
        uptime: "01:24:36",
    },
    launch: LaunchSurface {
        title: "Persistent agent chat",
        subtitle: "Daemon-owned single-agent surface · opened standalone or through Auspex",
        policy_owner: "auspex",
    },
    transcript: TRANSCRIPT,
    plan: PLAN,
    operations: OPERATIONS,
    daemon_events: DAEMON_EVENTS,
    context_spark: CONTEXT_SPARK,
    menu: MENU_SURFACE,
    modal_surfaces: MODAL_SURFACES,
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
/// One plan item: status drives the glyph + accent, intent tags the work kind,
/// progress fills inward. Mirrors PlanItemProjection.
#[component]
fn PlanRow(item: PlanItem) -> Element {
    let glyph = match item.status {
        "done" => "✓",
        "active" => "▸",
        "skipped" => "–",
        "blocked" => "!",
        _ => "·", // pending
    };
    rsx! {
        div { class: "owm-objective-row {item.status}",
            span { "{glyph}" }
            strong { "{item.label}" }
            em { class: "owm-plan-intent", "{item.intent}" }
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
        div { class: "owm-runtime-display owm-link-instrument",
            span { class: "owm-display-label", "LINK" }
            div { class: "owm-link-gauge-row",
                div { class: "owm-link-bars",
                    i {}
                    i {}
                    i {}
                    i {}
                }
                strong { class: "owm-display-value", "{latency}" }
            }
            span { class: "owm-link-gauge-state", "{status}" }
        }
    }
}

/// Uptime rendered as a clock instrument, not another text readout.
#[component]
fn UptimeGauge(uptime: &'static str) -> Element {
    rsx! {
        div { class: "owm-runtime-display owm-clock-instrument",
            span { class: "owm-display-label", "UPTIME" }
            strong { class: "owm-clock-value", "{uptime}" }
            div { class: "owm-clock-ticks",
                i {}
                i {}
                i {}
                i {}
                i {}
                i {}
            }
        }
    }
}

/// Session state rendered as a beacon/display, not a plain status pill.
#[component]
fn StateIndicator(status: &'static str) -> Element {
    rsx! {
        div { class: "owm-runtime-display owm-state-instrument owm-state-{status}",
            span { class: "owm-state-lamp" }
            div {
                span { class: "owm-display-label", "STATE" }
                strong { class: "owm-state-value", "{status}" }
            }
        }
    }
}

/// HUD top strip: mark · instrument readouts · global controls.
/// Replaces the former SaaS title/nav header.
#[component]
fn TopBar(
    launch: LaunchSurface,
    runtime: RuntimeSurface,
    session: SessionDescriptor,
    status: &'static str,
    on_open: EventHandler<ModalTarget>,
) -> Element {
    rsx! {
        header { class: "omegon-web-topbar",
            // Left HUD bank: identity + session context, consolidated as one module
            div { class: "owm-hud-bank owm-bank-left",
                div { class: "owm-hud-cell owm-cell-mark", title: "{launch.title} · session {session.session_id}",
                    div { class: "owm-core-glyph owm-mark-glyph" }
                    span { class: "owm-mark-id", "{runtime.agent_id}" }
                }
                HudReadout { label: "SESSION", value: session.session_id.to_string(), modifier: "owm-readout-mono" }
                HudReadout { label: "WORKSPACE", value: runtime.workspace.to_string(), modifier: "owm-readout-path" }
                HudReadout { label: "MODEL", value: runtime.model.to_string(), modifier: "owm-readout-live" }
            }

            // Center emblem — placeholder mount for the Omegon mark/logo
            div { class: "owm-hud-emblem", title: "Omegon",
                div { class: "owm-emblem-ring" }
                div { class: "owm-emblem-core" }
                span { class: "owm-emblem-label", "OMEGON" }
            }

            // Right HUD bank: live telemetry + global controls
            div { class: "owm-hud-bank owm-bank-right",
                LinkGauge { status: runtime.link_status, latency: runtime.latency }
                UptimeGauge { uptime: runtime.uptime }
                StateIndicator { status }
                HudReadout { label: "AUTONOMY", value: runtime.autonomy.to_string(), modifier: "owm-readout-autonomy" }
                div { class: "owm-hud-controls",
                    button {
                        class: "owm-hud-knob",
                        title: "Command palette",
                        onclick: move |_| on_open.call(ModalTarget::Commands),
                        "⌘"
                    }
                    button {
                        class: "owm-hud-knob",
                        title: "Settings",
                        onclick: move |_| on_open.call(ModalTarget::Settings),
                        "⚙"
                    }
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
fn PlanCard(plan: PlanLane) -> Element {
    rsx! {
        InstrumentCard { modifier: "owm-objectives-card", eyebrow: "PLAN",
            div { class: "owm-plan-head",
                span { class: "owm-plan-mode", "{plan.mode}" }
                span { class: "owm-plan-count", "{plan.completed}/{plan.total}" }
            }
            div { class: "owm-objective-stack",
                for item in plan.items.iter() {
                    PlanRow { item: *item }
                }
            }
        }
    }
}

/// One running/finished operation child. Mirrors OperationChildProjection.
#[component]
fn OperationRow(child: OperationChild) -> Element {
    rsx! {
        div { class: "owm-op-row {child.status}",
            div { class: "owm-op-row-head",
                strong { "{child.label}" }
                em { "{child.progress}" }
            }
            span { class: "owm-op-activity", "{child.activity}" }
            i { class: "owm-op-bar", style: "--p: {child.progress_pct}" }
        }
    }
}

/// Operations instrument: live delegate / cleave / background work.
/// Mirrors OperationProjection (running · completed · failed + children).
#[component]
fn OperationsCard(operations: OperationSurface) -> Element {
    rsx! {
        InstrumentCard { modifier: "owm-operations-card", eyebrow: "OPERATIONS",
            div { class: "owm-op-head",
                span { class: "owm-op-kind", "{operations.kind}" }
                div { class: "owm-op-counts",
                    span { class: "owm-op-running", "{operations.running} running" }
                    span { class: "owm-op-done", "{operations.completed} done" }
                    span { class: "owm-op-failed", "{operations.failed} failed" }
                }
            }
            div { class: "owm-op-stack",
                for child in operations.children.iter() {
                    OperationRow { child: *child }
                }
            }
        }
    }
}

/// Daemon/app event stream instrument. Mirrors `/api/events` snapshot plus the
/// `/api/events/stream` SSE feed; this is operational activity, not chat.
#[component]
fn DaemonEventsCard(events: DaemonEventsSurface) -> Element {
    rsx! {
        InstrumentCard { modifier: "owm-events-card", eyebrow: "EVENT STREAM",
            div { class: "owm-events-head",
                span { "{events.queued} queued" }
                span { "{events.processed} processed" }
            }
            div { class: "owm-event-endpoints",
                code { "{events.stream_href}" }
                code { "snapshot {events.snapshot_href}" }
            }
            div { class: "owm-daemon-event-stack",
                for event in events.events.iter() {
                    div { class: "owm-daemon-event {event.lane}",
                        span { class: "owm-daemon-event-type", "{event.event_type}" }
                        strong { "{event.summary}" }
                        em { "{event.age}" }
                    }
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
    on_expand: EventHandler<TranscriptEvent>,
) -> Element {
    // Tool calls collapse to a single compact, clickable row; the full payload
    // opens in the expansion modal rather than inflating the transcript.
    if event.role == "tool" {
        return rsx! {
            button {
                class: "owm-transcript-card owm-tool-row tool",
                onclick: move |_| on_expand.call(event),
                div { class: "owm-event-head",
                    strong { "{event.label}" }
                    span { "{event.meta}" }
                }
                p { class: "owm-tool-summary", "{event.body}" }
                span { class: "owm-tool-expand", "expand ⤢" }
            }
        };
    }
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
    on_open: EventHandler<ModalTarget>,
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
                    onclick: move |_| on_open.call(ModalTarget::Settings),
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

/// Tool-call expansion modal: full payload for a single tool entry.
#[component]
fn ToolModal(event: TranscriptEvent, on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "owm-modal-scrim", onclick: move |_| on_close.call(()),
            div { class: "owm-modal owm-tool-modal", onclick: move |e| e.stop_propagation(),
                div { class: "owm-modal-head",
                    div {
                        Eyebrow { label: "TOOL CALL" }
                        h2 { "{event.label}" }
                    }
                    button { class: "owm-ghost-button", onclick: move |_| on_close.call(()), "Close" }
                }
                div { class: "owm-modal-meta", "{event.meta}" }
                pre { class: "owm-tool-detail", "{event.detail}" }
            }
        }
    }
}

#[component]
fn MenuDock(menu: MenuSurface, on_open: EventHandler<ModalTarget>) -> Element {
    rsx! {
        div { class: "owm-menu-dock",
            for item in menu.items.iter() {
                button {
                    class: "owm-menu-card",
                    onclick: move |_| on_open.call(item.target),
                    span { class: "owm-menu-eyebrow", "{item.eyebrow}" }
                    strong { "{item.label}" }
                    p { "{item.summary}" }
                    em { "{item.signal}" }
                }
            }
        }
    }
}

fn modal_for(target: ModalTarget, surfaces: &'static [ModalSurface]) -> ModalSurface {
    surfaces
        .iter()
        .copied()
        .find(|surface| surface.target == target)
        .unwrap_or(surfaces[0])
}

#[component]
fn SemanticModal(surface: ModalSurface, on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "owm-modal-scrim", onclick: move |_| on_close.call(()),
            section { class: "owm-modal-card owm-semantic-modal", onclick: move |event| event.stop_propagation(),
                div { class: "owm-modal-head",
                    div {
                        Eyebrow { label: surface.eyebrow }
                        h2 { "{surface.title}" }
                    }
                    button {
                        class: "owm-ghost-button owm-close-button",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }
                p { class: "owm-modal-summary", "{surface.summary}" }
                div { class: "owm-modal-section-grid",
                    for section in surface.sections.iter() {
                        article { class: "owm-modal-section",
                            span { "{section.label}" }
                            strong { "{section.value}" }
                            p { "{section.detail}" }
                        }
                    }
                }
            }
        }
    }
}

// ============================================================
// App root — wires state and composes the surfaces.
// ============================================================

#[component]
pub fn OmegonWebMockApp() -> Element {
    let backend_session = crate::omegon_web_contract::fixture_session();
    let launch_context = crate::omegon_web_contract::proxied_launch_context_fixture();
    let surface = MOCK_SURFACE;
    debug_assert_eq!(backend_session.schema_version, surface.session.schema_version);
    debug_assert_eq!(backend_session.session.session_id, surface.session.session_id);
    debug_assert_eq!(launch_context.policy_owner, surface.launch.policy_owner);
    debug_assert_eq!(
        backend_session.links.surfaces.as_deref(),
        surface.session.links.surfaces
    );
    debug_assert_eq!(
        backend_session.links.actions.as_deref(),
        surface.session.links.actions
    );
    debug_assert_eq!(backend_session.links.stream.as_deref(), surface.session.links.stream);
    let composer = use_signal(|| String::from(surface.composer.initial_prompt));
    let mut modal_target = use_signal(|| Option::<ModalTarget>::None);
    let mut approval_state = use_signal(|| "pending");
    let sent_count = use_signal(|| 0_u32);
    let mut tool_modal = use_signal(|| Option::<TranscriptEvent>::None);

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
                session: surface.session,
                status,
                on_open: move |target| modal_target.set(Some(target)),
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
                                    on_expand: move |ev| tool_modal.set(Some(ev)),
                                }
                            }
                        }
                    }

                    MenuDock {
                        menu: surface.menu,
                        on_open: move |target| modal_target.set(Some(target)),
                    }

                    Composer {
                        queue_mode: surface.composer.queue_mode,
                        composer,
                        sent_count,
                        on_open: move |target| modal_target.set(Some(target)),
                    }
                }

                aside { class: "owm-cockpit-rail owm-right-rail",
                    PlanCard { plan: surface.plan }
                    OperationsCard { operations: surface.operations }
                    DaemonEventsCard { events: surface.daemon_events }
                }
            }

            if let Some(ev) = tool_modal.read().clone() {
                ToolModal { event: ev, on_close: move |_| tool_modal.set(None) }
            }

            if let Some(target) = *modal_target.read() {
                SemanticModal {
                    surface: modal_for(target, surface.modal_surfaces),
                    on_close: move |_| modal_target.set(None),
                }
            }
        }
    }
}
