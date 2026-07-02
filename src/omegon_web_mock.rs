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

#[derive(Clone, PartialEq)]
struct SessionLinks {
    surfaces: Option<String>,
    actions: Option<String>,
    stream: Option<String>,
}

#[derive(Clone, PartialEq)]
struct SessionDescriptor {
    schema_version: u8,
    session_id: String,
    current: bool,
    assistant_profile_id: Option<String>,
    assistant_readiness: Option<String>,
    links: SessionLinks,
}

#[derive(Clone, PartialEq)]
struct WebSurfaceSnapshot {
    session: SessionDescriptor,
    runtime: RuntimeSurface,
    launch: LaunchSurface,
    transcript: Vec<TranscriptEvent>,
    plan: PlanLane,
    operations: OperationSurface,
    daemon_events: DaemonEventsSurface,
    context_spark: SparklineSpec,
    menu: MenuSurface,
    modal_surfaces: &'static [ModalSurface],
    composer: ComposerSurface,
}

#[derive(Clone, PartialEq)]
struct RuntimeSurface {
    agent_id: String,
    state: String,
    workspace: String,
    model: String,
    context_window: String,
    tool_count: u16,
    tool_online: u16,
    tool_sockets: u16,
    transport: String,
    link_status: String,
    latency: String,
    autonomy: String,
    uptime: String,
}

#[derive(Clone, PartialEq)]
struct LaunchSurface {
    title: String,
    subtitle: String,
    policy_owner: String,
}

#[derive(Clone, PartialEq)]
struct ComposerSurface {
    queue_mode: String,
    initial_prompt: String,
}

#[derive(Clone, PartialEq)]
struct TranscriptEvent {
    role: String,
    label: String,
    body: String,
    meta: String,
    /// Present on tool calls: full payload shown in the expansion modal.
    detail: String,
}

/// Mirrors `omegon_traits::PlanItemProjection`: status is one of
/// pending|active|done|skipped; intent tags the kind of work.
#[derive(Clone, PartialEq)]
struct PlanItem {
    status: String,
    intent: String,
    label: String,
    progress: String,
}

/// Mirrors `omegon_traits::PlanLaneProjection`: a mode + progress + items.
#[derive(Clone, PartialEq)]
struct PlanLane {
    mode: String,
    completed: usize,
    total: usize,
    items: Vec<PlanItem>,
}

/// Mirrors `omegon_traits::OperationChildProjection`.
#[derive(Clone, PartialEq)]
struct OperationChild {
    label: String,
    status: String,
    activity: String,
    progress: String,
    progress_pct: String,
}

/// Mirrors `omegon_traits::OperationProjection`: delegate / cleave / background.
#[derive(Clone, PartialEq)]
struct OperationSurface {
    kind: String,
    running: usize,
    completed: usize,
    failed: usize,
    children: Vec<OperationChild>,
}

/// Mirrors `/api/events/stream` daemon/app SSE payloads.
#[derive(Clone, PartialEq)]
struct DaemonEventItem {
    event_type: String,
    lane: String,
    summary: String,
    age: String,
}

/// Mirrors the daemon event snapshot/stream pair: `/api/events` + `/api/events/stream`.
#[derive(Clone, PartialEq)]
struct DaemonEventsSurface {
    queued: usize,
    processed: usize,
    stream_href: String,
    snapshot_href: String,
    events: Vec<DaemonEventItem>,
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

fn default_daemon_events() -> DaemonEventsSurface {
    DaemonEventsSurface {
        queued: 2,
        processed: 47,
        stream_href: "/api/events/stream".to_string(),
        snapshot_href: "/api/events".to_string(),
        events: vec![
            DaemonEventItem {
                event_type: "runtime.context_changed".to_string(),
                lane: "runtime".to_string(),
                summary: "context window recalculated: 82k / 128k tokens".to_string(),
                age: "12s".to_string(),
            },
            DaemonEventItem {
                event_type: "lifecycle.snapshot_changed".to_string(),
                lane: "plan".to_string(),
                summary: "plan projection updated; active lane still executing".to_string(),
                age: "33s".to_string(),
            },
            DaemonEventItem {
                event_type: "provider.status_changed".to_string(),
                lane: "provider".to_string(),
                summary: "openai-codex:gpt-5.5 remains selected and serving".to_string(),
                age: "1m".to_string(),
            },
            DaemonEventItem {
                event_type: "stream.lagged".to_string(),
                lane: "recovery".to_string(),
                summary: "client skipped events; refetch snapshot from /api/events".to_string(),
                age: "3m".to_string(),
            },
        ],
    }
}

fn normalize_backend_session(
    session: crate::omegon_web_contract::BackendSessionShowResponse,
    launch_context: crate::omegon_web_contract::BackendLaunchContextResponse,
) -> WebSurfaceSnapshot {
    let surfaces = session.snapshot.surfaces;
    let active_tool_count = surfaces
        .instruments
        .tools
        .iter()
        .filter(|tool| tool.status == "running")
        .count() as u16;
    let tool_count = surfaces.instruments.tools.len() as u16;
    let tool_sockets = tool_count.max(12);
    let stream_href = session
        .links
        .stream
        .clone()
        .unwrap_or_else(|| "/api/sessions/default/surfaces/stream".to_string());
    let context_window = format!(
        "{} facts · {} turns",
        surfaces.memory_status.active_facts, surfaces.dashboard.session.turns
    );
    let mut transcript: Vec<TranscriptEvent> = surfaces
        .conversation
        .segments
        .into_iter()
        .map(|segment| TranscriptEvent {
            role: match segment.role.as_str() {
                "user" => "operator".to_string(),
                other => other.to_string(),
            },
            label: segment
                .title
                .unwrap_or_else(|| match segment.role.as_str() {
                    "user" => "Operator".to_string(),
                    "assistant" => "Omegon".to_string(),
                    other => other.to_string(),
                }),
            body: segment.body.or(segment.summary).unwrap_or_default(),
            meta: if segment.complete {
                "conversation · complete"
            } else {
                "conversation · streaming"
            }
            .to_string(),
            detail: String::new(),
        })
        .collect();
    transcript.extend(surfaces.instruments.tools.iter().map(|tool| {
        let body = tool
            .result_summary
            .clone()
            .or_else(|| tool.output_tail.clone())
            .unwrap_or_else(|| format!("{} {}", tool.name, tool.status));
        let args = serde_json::to_string_pretty(&tool.args).unwrap_or_else(|_| "{}".to_string());
        let mut detail = format!("tool {} · {}\nargs:\n{}", tool.name, tool.status, args);
        if let Some(output_tail) = &tool.output_tail {
            detail.push_str("\n\noutput tail:\n");
            detail.push_str(output_tail);
        }
        if let Some(summary) = &tool.result_summary {
            detail.push_str("\n\nsummary:\n");
            detail.push_str(summary);
        }
        TranscriptEvent {
            role: "tool".to_string(),
            label: format!("Tool · {}", tool.name),
            body,
            meta: format!(
                "{} · {} ms",
                tool.phase.clone().unwrap_or_else(|| "tool".to_string()),
                tool.elapsed_ms.unwrap_or(0)
            ),
            detail,
        }
    }));

    let active_plan = surfaces.plan.active;
    WebSurfaceSnapshot {
        session: SessionDescriptor {
            schema_version: session.schema_version,
            session_id: session.session.session_id,
            current: session.session.current,
            assistant_profile_id: None,
            assistant_readiness: Some("ready".to_string()),
            links: SessionLinks {
                surfaces: session.links.surfaces,
                actions: session.links.actions,
                stream: session.links.stream,
            },
        },
        runtime: RuntimeSurface {
            agent_id: "daemon-01".to_string(),
            state: if session.session.current {
                "attached"
            } else {
                "archived"
            }
            .to_string(),
            workspace: session.session.cwd,
            model: surfaces
                .runtime
                .capability_grade
                .clone()
                .unwrap_or_else(|| "daemon snapshot".to_string()),
            context_window,
            tool_count,
            tool_online: active_tool_count,
            tool_sockets: tool_sockets.max(1),
            transport: stream_href,
            link_status: "STREAM NOMINAL".to_string(),
            latency: "fixture".to_string(),
            autonomy: surfaces
                .runtime
                .autonomy_mode
                .unwrap_or_else(|| "Conservative".to_string())
                .to_lowercase(),
            uptime: format!("{}t", session.session.turns),
        },
        launch: LaunchSurface {
            title: "Persistent agent chat".to_string(),
            subtitle: "Daemon-owned single-agent surface · opened standalone or through Auspex"
                .to_string(),
            policy_owner: launch_context.policy_owner,
        },
        transcript,
        plan: active_plan
            .map(|plan| PlanLane {
                mode: plan.mode,
                completed: plan.completed,
                total: plan.total,
                items: plan
                    .items
                    .into_iter()
                    .map(|item| PlanItem {
                        progress: match item.status.as_str() {
                            "done" => "100%".to_string(),
                            "active" => "42%".to_string(),
                            _ => "0%".to_string(),
                        },
                        status: item.status,
                        intent: item.intent.unwrap_or_else(|| "work".to_string()),
                        label: item.label,
                    })
                    .collect(),
            })
            .unwrap_or_else(|| PlanLane {
                mode: "idle".to_string(),
                completed: 0,
                total: 0,
                items: Vec::new(),
            }),
        operations: OperationSurface {
            kind: surfaces
                .operations
                .kind
                .unwrap_or_else(|| "idle".to_string()),
            running: surfaces.operations.running,
            completed: surfaces.operations.completed,
            failed: surfaces.operations.failed,
            children: surfaces
                .operations
                .children
                .into_iter()
                .map(|child| {
                    let progress_pct = if child.tasks_total == 0 {
                        0
                    } else {
                        child.tasks_done.saturating_mul(100) / child.tasks_total
                    };
                    OperationChild {
                        label: child.label,
                        status: child.status,
                        activity: child
                            .activity
                            .or(child.result_summary)
                            .unwrap_or_else(|| "idle".to_string()),
                        progress: format!("{}/{}", child.tasks_done, child.tasks_total),
                        progress_pct: format!("{progress_pct}%"),
                    }
                })
                .collect(),
        },
        daemon_events: default_daemon_events(),
        context_spark: CONTEXT_SPARK,
        menu: MENU_SURFACE,
        modal_surfaces: MODAL_SURFACES,
        composer: ComposerSurface {
            queue_mode: surfaces.editor.queue_mode,
            initial_prompt: surfaces.editor.placeholder,
        },
    }
}

fn with_bootstrap_status(
    mut surface: WebSurfaceSnapshot,
    status: &str,
    latency: &str,
) -> WebSurfaceSnapshot {
    surface.runtime.link_status = status.to_string();
    surface.runtime.latency = latency.to_string();
    surface
}

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
    ModalSection {
        label: "Session",
        value: "default · current",
        detail: "Native session envelope from POST /api/sessions; singleton for phase 1.",
    },
    ModalSection {
        label: "Proxy",
        value: "auspex · trusted",
        detail: "Auspex forwards standard Omegon-Principal-* headers plus Omegon-Back-Url.",
    },
    ModalSection {
        label: "Policy owner",
        value: "auspex",
        detail: "RBAC decisions are evaluated by Omegon using the trusted Auspex principal.",
    },
    ModalSection {
        label: "Transport",
        value: "/api/sessions/default/surfaces/stream",
        detail: "Native session stream; legacy /api/web/surfaces remains a compatibility path.",
    },
    ModalSection {
        label: "Autonomy",
        value: "conservative",
        detail: "Operator confirmations required for elevated or ambiguous actions.",
    },
];

const CONFIG_SECTIONS: &[ModalSection] = &[
    ModalSection {
        label: "Bootstrap",
        value: "POST /api/sessions",
        detail: "Create or attach first, then follow daemon-provided links instead of hardcoded web paths.",
    },
    ModalSection {
        label: "Snapshot",
        value: "/api/sessions/default/surfaces",
        detail: "HTTP bootstrap for the current semantic surface bundle.",
    },
    ModalSection {
        label: "Actions",
        value: "/api/sessions/default/actions",
        detail: "Prompt sends, approvals, and UI actions post here; daemon remains authoritative.",
    },
    ModalSection {
        label: "Transcript mode",
        value: "chat + collapsed tools",
        detail: "Operator/agent prose stays in the feed; tool payloads expand in modal overlays.",
    },
];

const ARMORY_SECTIONS: &[ModalSection] = &[
    ModalSection {
        label: "Shell",
        value: "enabled · gated",
        detail: "Command execution is available through policy prompts and audit entries.",
    },
    ModalSection {
        label: "Files",
        value: "read/write scoped",
        detail: "Workspace edits remain constrained to approved project roots.",
    },
    ModalSection {
        label: "Subagents",
        value: "delegate · cleave",
        detail: "Background workers surface as Operations rows instead of transcript noise.",
    },
];

const COMMAND_SECTIONS: &[ModalSection] = &[
    ModalSection {
        label: "/continue",
        value: "resume plan",
        detail: "Continue from the active plan item.",
    },
    ModalSection {
        label: "/plan status",
        value: "inspect",
        detail: "Open the current plan lane and workstream summary.",
    },
    ModalSection {
        label: "/tools",
        value: "armory",
        detail: "Inspect available tool surfaces and safety gates.",
    },
];

const MODAL_SURFACES: &[ModalSurface] = &[
    ModalSurface {
        target: ModalTarget::Settings,
        eyebrow: "SETTINGS",
        title: "Session settings",
        summary: "Session authority and transport controls. These are global levers, not chat content.",
        sections: SETTINGS_SECTIONS,
    },
    ModalSurface {
        target: ModalTarget::Config,
        eyebrow: "CONFIG",
        title: "Surface configuration",
        summary: "Renderer and data-source settings for the web cockpit.",
        sections: CONFIG_SECTIONS,
    },
    ModalSurface {
        target: ModalTarget::Armory,
        eyebrow: "ARMORY",
        title: "Capability armory",
        summary: "Curated tool/capability inventory with safety posture attached.",
        sections: ARMORY_SECTIONS,
    },
    ModalSurface {
        target: ModalTarget::Commands,
        eyebrow: "COMMANDS",
        title: "Command palette",
        summary: "Operator shortcuts and command-shaped intents.",
        sections: COMMAND_SECTIONS,
    },
];

/// Tick + label + extending-rule section header.
#[component]
fn Eyebrow(label: &'static str) -> Element {
    rsx! { div { class: "owm-eyebrow", "{label}" } }
}

/// Amber signal chip for live status words.
#[component]
fn StateChip(label: String) -> Element {
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
    let glyph = match item.status.as_str() {
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
fn LinkGauge(status: String, latency: String) -> Element {
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
fn UptimeGauge(uptime: String) -> Element {
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
fn StateIndicator(status: String) -> Element {
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
    status: String,
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
                    PlanRow { item: item.clone() }
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
                    OperationRow { child: child.clone() }
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
        let expanded_event = event.clone();
        return rsx! {
            button {
                class: "owm-transcript-card owm-tool-row tool",
                onclick: move |_| on_expand.call(expanded_event.clone()),
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
    queue_mode: String,
    composer: Signal<String>,
    sent_count: Signal<u32>,
    submit_status: Signal<String>,
    action_endpoint: Option<String>,
    session_id: String,
    client_id: String,
    on_open: EventHandler<ModalTarget>,
) -> Element {
    let mut composer = composer;
    let mut sent_count = sent_count;
    let mut submit_status = submit_status;
    rsx! {
        section { class: "owm-panel owm-composer-panel",
            div { class: "owm-composer-meta",
                span { "queue mode: {queue_mode}" }
                span { "sent: {sent_count}" }
                span { "action: {submit_status}" }
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
                        let text = composer.read().trim().to_string();
                        if text.is_empty() {
                            submit_status.set("empty prompt".to_string());
                            return;
                        }
                        let Some(endpoint) = action_endpoint.clone() else {
                            submit_status.set("no action endpoint".to_string());
                            return;
                        };
                        let action_id = format!("auspex-web-{}", *sent_count.read() + 1);
                        let request = crate::omegon_web_contract::submit_prompt_action(
                            action_id,
                            client_id.clone(),
                            session_id.clone(),
                            text,
                            Vec::new(),
                        );
                        let principal = crate::omegon_web_contract::TrustedPrincipalHeaders::auspex_operator("operator:web")
                            .display_name("Web Operator")
                            .session_id(session_id.clone())
                            .client_id(client_id.clone());
                        submit_status.set("posting".to_string());
                        spawn(async move {
                            match crate::omegon_web_contract::post_action_request(&endpoint, &request, Some(&principal)).await {
                                Ok(outcome) => {
                                    let status = format!("{:?}", outcome.status).to_lowercase();
                                    let accepted = outcome.error.is_none();
                                    submit_status.set(outcome.message.unwrap_or(status));
                                    if accepted {
                                        let next = *sent_count.read() + 1;
                                        sent_count.set(next);
                                        composer.set(String::new());
                                    }
                                }
                                Err(error) => submit_status.set(error.to_string()),
                            }
                        });
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
    let surface_resource =
        use_resource(|| async { crate::omegon_web_contract::load_initial_session().await });
    let fallback_session = crate::omegon_web_contract::fixture_session();
    let fallback_launch = crate::omegon_web_contract::proxied_launch_context_fixture();

    debug_assert_eq!(fallback_session.schema_version, 1);
    debug_assert_eq!(fallback_session.session.session_id, "default");
    debug_assert_eq!(fallback_launch.policy_owner, "auspex");
    debug_assert_eq!(
        fallback_session.links.surfaces.as_deref(),
        Some("/api/sessions/default/surfaces")
    );
    debug_assert_eq!(
        fallback_session.links.actions.as_deref(),
        Some("/api/sessions/default/actions")
    );
    debug_assert_eq!(
        fallback_session.links.stream.as_deref(),
        Some("/api/sessions/default/surfaces/stream")
    );

    let surface = {
        let loaded = surface_resource.read();
        match loaded.as_ref() {
            Some(Ok((session, launch_context))) => with_bootstrap_status(
                normalize_backend_session(session.clone(), launch_context.clone()),
                "STREAM LIVE",
                "live",
            ),
            Some(Err(_error)) => with_bootstrap_status(
                normalize_backend_session(fallback_session, fallback_launch),
                "FIXTURE FALLBACK",
                "offline",
            ),
            None => with_bootstrap_status(
                normalize_backend_session(fallback_session, fallback_launch),
                "CONNECTING",
                "boot",
            ),
        }
    };
    let composer = use_signal(|| String::from(surface.composer.initial_prompt));
    let mut modal_target = use_signal(|| Option::<ModalTarget>::None);
    let mut approval_state = use_signal(|| "pending");
    let sent_count = use_signal(|| 0_u32);
    let submit_status = use_signal(|| "idle".to_string());
    let mut tool_modal = use_signal(|| Option::<TranscriptEvent>::None);
    let action_endpoint = surface.session.links.actions.clone();
    let session_id = surface.session.session_id.clone();
    let client_id = "auspex-web".to_string();

    let status = if *approval_state.read() == "pending" {
        "waiting".to_string()
    } else {
        "running".to_string()
    };

    rsx! {
        div { class: "omegon-web-shell",
            div { class: "omegon-web-bg" }
            div { class: "hud-frame" }

            TopBar {
                launch: surface.launch,
                runtime: surface.runtime.clone(),
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
                                    event: event.clone(),
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
                        queue_mode: surface.composer.queue_mode.clone(),
                        composer,
                        sent_count,
                        submit_status,
                        action_endpoint,
                        session_id,
                        client_id,
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
