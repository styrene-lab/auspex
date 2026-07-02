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
struct CommandItem {
    command: &'static str,
    label: &'static str,
    group: &'static str,
    detail: &'static str,
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
    backend: BackendLinkSurface,
    daemon_events: DaemonEventsSurface,
    modal_surfaces: &'static [ModalSurface],
    composer: ComposerSurface,
}

#[derive(Clone, PartialEq)]
struct RuntimeSurface {
    agent_id: String,
    state: String,
    workspace: String,
    model: String,
    posture: String,
    thinking: String,
    branch: String,
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
    placeholder: String,
}

#[derive(Clone, PartialEq)]
struct SurfaceRefreshContext {
    session_schema_version: u8,
    session_id: String,
    current: bool,
    cwd: String,
    description: String,
    links: SessionLinks,
    launch: LaunchSurface,
    surfaces_endpoint: Option<String>,
    stream_endpoint: Option<String>,
}

#[derive(Clone, PartialEq)]
struct TranscriptEvent {
    role: String,
    label: String,
    body: String,
    meta: String,
    request_id: Option<String>,
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

#[derive(Clone, PartialEq)]
struct BackendLinkSurface {
    surfaces_href: String,
    actions_href: String,
    stream_href: String,
    revision: u64,
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

// ---- Mock data ---------------------------------------------

/// Split camel-case daemon enums into readable words:
/// "GuardedAutonomous" → "guarded autonomous".
fn prettify_mode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 4);
    for (index, ch) in raw.chars().enumerate() {
        if ch.is_uppercase() && index > 0 {
            out.push(' ');
        }
        out.extend(ch.to_lowercase());
    }
    out.replace('_', " ")
}

/// Honest empty default — real numbers arrive from `GET /api/events`.
fn empty_daemon_events() -> DaemonEventsSurface {
    DaemonEventsSurface {
        queued: 0,
        processed: 0,
        stream_href: "/api/events/stream".to_string(),
        snapshot_href: "/api/events".to_string(),
        events: Vec::new(),
    }
}

/// Normalize raw daemon event payloads from `/api/events` into display items.
/// Payload shape is daemon-versioned; extract defensively.
fn normalize_daemon_events(
    response: &crate::omegon_web_contract::BackendDaemonEventsResponse,
) -> DaemonEventsSurface {
    let events = response
        .events
        .iter()
        .rev()
        .take(6)
        .map(|value| {
            let event_type = value
                .get("type")
                .or_else(|| value.get("event_type"))
                .or_else(|| value.get("kind"))
                .and_then(|v| v.as_str())
                .unwrap_or("event")
                .to_string();
            let summary = value
                .get("summary")
                .or_else(|| value.get("message"))
                .or_else(|| value.get("detail"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    let raw = value.to_string();
                    if raw.len() > 96 {
                        format!("{}…", &raw[..96])
                    } else {
                        raw
                    }
                });
            let lane = match event_type.split('.').next().unwrap_or("") {
                "runtime" | "session" => "runtime",
                "lifecycle" | "plan" => "plan",
                "provider" | "model" => "provider",
                "stream" | "recovery" | "error" => "recovery",
                _ => "runtime",
            }
            .to_string();
            let age = value
                .get("age")
                .or_else(|| value.get("ts"))
                .or_else(|| value.get("timestamp"))
                .and_then(|v| v.as_str())
                .unwrap_or("—")
                .to_string();
            DaemonEventItem {
                event_type,
                lane,
                summary,
                age,
            }
        })
        .collect();
    DaemonEventsSurface {
        queued: response.queued_events,
        processed: response.processed_events,
        stream_href: "/api/events/stream".to_string(),
        snapshot_href: "/api/events".to_string(),
        events,
    }
}

fn normalize_backend_session(
    session: crate::omegon_web_contract::BackendSessionShowResponse,
    launch_context: crate::omegon_web_contract::BackendLaunchContextResponse,
) -> WebSurfaceSnapshot {
    let surfaces = session.snapshot.surfaces;
    let snapshot_revision = session.snapshot.revision;
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
            request_id: segment.request_id,
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
            request_id: None,
            detail,
        }
    }));

    let active_plan = surfaces.plan.active;
    let session_links = SessionLinks {
        surfaces: session.links.surfaces,
        actions: session.links.actions,
        stream: session.links.stream,
    };
    let backend = BackendLinkSurface {
        surfaces_href: session_links
            .surfaces
            .clone()
            .unwrap_or_else(|| "offline".to_string()),
        actions_href: session_links
            .actions
            .clone()
            .unwrap_or_else(|| "read-only".to_string()),
        stream_href: session_links
            .stream
            .clone()
            .unwrap_or_else(|| "unavailable".to_string()),
        revision: snapshot_revision,
    };
    WebSurfaceSnapshot {
        session: SessionDescriptor {
            schema_version: session.schema_version,
            session_id: session.session.session_id,
            current: session.session.current,
            assistant_profile_id: None,
            assistant_readiness: Some("ready".to_string()),
            links: session_links,
        },
        runtime: RuntimeSurface {
            agent_id: "daemon-01".to_string(),
            state: if surfaces.footer.busy {
                "busy"
            } else if session.session.current {
                "ready"
            } else {
                "archived"
            }
            .to_string(),
            workspace: session.session.cwd,
            model: format!(
                "{} · {}",
                surfaces
                    .runtime
                    .capability_grade
                    .clone()
                    .unwrap_or_else(|| "?".to_string()),
                surfaces
                    .runtime
                    .context_class
                    .clone()
                    .unwrap_or_else(|| "unknown class".to_string()),
            ),
            posture: surfaces
                .runtime
                .posture
                .clone()
                .unwrap_or_else(|| "—".to_string()),
            thinking: surfaces
                .runtime
                .thinking_level
                .clone()
                .unwrap_or_else(|| "—".to_string()),
            branch: surfaces
                .runtime
                .git_branch
                .clone()
                .unwrap_or_else(|| "—".to_string()),
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
                .as_deref()
                .map(prettify_mode)
                .unwrap_or_else(|| "conservative".to_string()),
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
        backend,
        daemon_events: empty_daemon_events(),
        modal_surfaces: MODAL_SURFACES,
        composer: ComposerSurface {
            queue_mode: surfaces.editor.queue_mode,
            placeholder: surfaces.editor.placeholder,
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

fn refresh_context_for(surface: &WebSurfaceSnapshot) -> SurfaceRefreshContext {
    SurfaceRefreshContext {
        session_schema_version: surface.session.schema_version,
        session_id: surface.session.session_id.clone(),
        current: surface.session.current,
        cwd: surface.runtime.workspace.clone(),
        description: if surface.session.current {
            "Current live session".to_string()
        } else {
            "Historical session".to_string()
        },
        links: surface.session.links.clone(),
        launch: surface.launch.clone(),
        surfaces_endpoint: surface.session.links.surfaces.clone(),
        stream_endpoint: surface.session.links.stream.clone(),
    }
}

fn normalize_refreshed_surface(
    context: &SurfaceRefreshContext,
    snapshot: crate::omegon_web_contract::BackendSurfacesSnapshot,
) -> WebSurfaceSnapshot {
    let turns = snapshot.surfaces.dashboard.session.turns;
    let tool_calls = snapshot.surfaces.dashboard.session.tool_calls;
    let session = crate::omegon_web_contract::BackendSessionShowResponse {
        schema_version: context.session_schema_version,
        session: crate::omegon_web_contract::BackendSessionSummary {
            session_id: context.session_id.clone(),
            cwd: context.cwd.clone(),
            created_at: snapshot.generated_at.clone(),
            turns,
            tool_calls,
            description: context.description.clone(),
            last_prompt_snippet: context.description.clone(),
            current: context.current,
        },
        allocation_mode: "singleton-live".to_string(),
        links: crate::omegon_web_contract::BackendSessionLinks {
            surfaces: context.links.surfaces.clone(),
            actions: context.links.actions.clone(),
            stream: context.links.stream.clone(),
        },
        snapshot,
    };
    let launch = crate::omegon_web_contract::BackendLaunchContextResponse {
        mode: "proxied".to_string(),
        proxied_by: Some("auspex".to_string()),
        back_url: None,
        policy_owner: context.launch.policy_owner.clone(),
    };
    normalize_backend_session(session, launch)
}

const COMMAND_ITEMS: &[CommandItem] = &[
    CommandItem {
        command: "/status",
        label: "Status",
        group: "Session",
        detail: "Show runtime, provider, and session state.",
    },
    CommandItem {
        command: "/model providers",
        label: "Model providers",
        group: "Model",
        detail: "Inspect configured providers and current route.",
    },
    CommandItem {
        command: "/model openai-codex:gpt-5.5",
        label: "Use Codex GPT-5.5",
        group: "Model",
        detail: "Switch the active model route when the backend permits it.",
    },
    CommandItem {
        command: "/context massive",
        label: "Massive context",
        group: "Runtime",
        detail: "Request the largest available context profile.",
    },
    CommandItem {
        command: "/compact",
        label: "Compact context",
        group: "Runtime",
        detail: "Ask Omegon to compact the current session context.",
    },
    CommandItem {
        command: "/tools",
        label: "Tools",
        group: "Inventory",
        detail: "Open tool inventory and capability status.",
    },
    CommandItem {
        command: "/skills",
        label: "Skills",
        group: "Inventory",
        detail: "Inspect active skills and loaded instruction bundles.",
    },
    CommandItem {
        command: "/clear",
        label: "Clear transcript",
        group: "Session",
        detail: "Clear visible conversation state if supported by the backend.",
    },
];

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
            div { class: "owm-objective-head",
                span { "{glyph}" }
                strong { "{item.label}" }
                em { class: "owm-plan-intent", "{item.intent}" }
            }
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
                    div { class: "owm-mark-glyph" }
                    span { class: "owm-mark-id", "{runtime.agent_id}" }
                }
                HudReadout { label: "SESSION", value: session.session_id.to_string(), modifier: "owm-readout-mono" }
                HudReadout { label: "WORKSPACE", value: runtime.workspace.to_string(), modifier: "owm-readout-path" }
                HudReadout { label: "MODEL", value: runtime.model.to_string(), modifier: "owm-readout-live" }
            }

            // Center emblem — placeholder mount for the Omegon mark/logo
            div { class: "owm-hud-emblem", title: "Omegon",
                div { class: "owm-emblem-ring",
                    div { class: "owm-emblem-core" }
                }
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
fn DaemonCoreCard(runtime: RuntimeSurface) -> Element {
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
            }
            div { class: "owm-socket-meter",
                MeterHead { label: "Tool sockets", value: format!("{} active · {} tools", runtime.tool_online, runtime.tool_count) }
                SocketGrid { online: runtime.tool_online, total: runtime.tool_sockets }
            }
            div { class: "owm-core-profile",
                div { class: "owm-profile-row",
                    span { "POSTURE" }
                    strong { "{runtime.posture}" }
                }
                div { class: "owm-profile-row",
                    span { "THINKING" }
                    strong { "{runtime.thinking}" }
                }
                div { class: "owm-profile-row",
                    span { "BRANCH" }
                    strong { "{runtime.branch}" }
                }
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
                if plan.items.is_empty() {
                    div { class: "owm-empty", "No active plan lane" }
                }
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
                if operations.children.is_empty() {
                    div { class: "owm-empty", "No delegate or cleave work running" }
                }
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
fn BackendStatusCard(
    backend: BackendLinkSurface,
    action_status: Signal<String>,
    stream_status: Signal<String>,
) -> Element {
    let (auth_state, auth_note) = if crate::omegon_web_contract::page_query_token().is_some() {
        ("bearer attached", "page ?token= → Authorization")
    } else {
        ("no token", "gated routes will 401 — open /?token=<daemon token>")
    };
    let auth_live = if crate::omegon_web_contract::page_query_token().is_some() {
        "owm-backend-row live"
    } else {
        "owm-backend-row"
    };
    rsx! {
        section { class: "owm-panel owm-backend-card",
            div { class: "owm-op-head",
                Eyebrow { label: "BACKEND" }
                span { class: "owm-op-kind", "rev {backend.revision}" }
            }
            div { class: "owm-backend-stack",
                div { class: "{auth_live}",
                    span { "auth" }
                    strong { "{auth_state}" }
                    em { "{auth_note}" }
                }
                div { class: "owm-backend-row live",
                    span { "stream" }
                    strong { "{stream_status}" }
                    em { "{backend.stream_href}" }
                }
                div { class: "owm-backend-row",
                    span { "actions" }
                    strong { "{action_status}" }
                    em { "{backend.actions_href}" }
                }
                div { class: "owm-backend-row",
                    span { "surfaces" }
                    strong { "snapshot refresh" }
                    em { "{backend.surfaces_href}" }
                }
            }
        }
    }
}

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
                if events.events.is_empty() {
                    div { class: "owm-empty", "Event stream quiet" }
                }
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
    action_endpoint: Option<String>,
    session_id: String,
    client_id: String,
    refresh_context: SurfaceRefreshContext,
    surface_override: Signal<Option<WebSurfaceSnapshot>>,
    submit_status: Signal<String>,
    approval_state: Signal<&'static str>,
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
                    span { class: "owm-tool-expand", "expand ⤢" }
                }
                p { class: "owm-tool-summary", "{event.body}" }
            }
        };
    }
    let deny_event = event.clone();
    let approve_event = event.clone();
    let deny_action_endpoint = action_endpoint.clone();
    let approve_action_endpoint = action_endpoint.clone();
    let deny_session_id = session_id.clone();
    let approve_session_id = session_id.clone();
    let deny_client_id = client_id.clone();
    let approve_client_id = client_id.clone();
    let deny_refresh_context = refresh_context.clone();
    let approve_refresh_context = refresh_context.clone();
    let pending = event.body.trim().is_empty() && event.meta.contains("streaming");
    let body = if pending {
        "Waiting for Omegon response…".to_string()
    } else if event.body.trim().is_empty() {
        "No content reported for this segment.".to_string()
    } else {
        event.body.clone()
    };
    let card_class = if pending {
        format!("owm-transcript-card {} pending", event.role)
    } else {
        format!("owm-transcript-card {}", event.role)
    };
    rsx! {
        article { class: "{card_class}",
            div { class: "owm-event-head",
                strong { "{event.label}" }
                span { "{event.meta}" }
            }
            p { "{body}" }
            if event.role == "approval" {
                div { class: "owm-approval-actions",
                    button {
                        class: "owm-danger-button",
                        onclick: move |_| submit_permission_response(
                            deny_event.clone(),
                            false,
                            deny_action_endpoint.clone(),
                            deny_session_id.clone(),
                            deny_client_id.clone(),
                            deny_refresh_context.clone(),
                            surface_override,
                            submit_status,
                            approval_state,
                        ),
                        "Deny"
                    }
                    button {
                        class: "owm-primary-button",
                        onclick: move |_| submit_permission_response(
                            approve_event.clone(),
                            true,
                            approve_action_endpoint.clone(),
                            approve_session_id.clone(),
                            approve_client_id.clone(),
                            approve_refresh_context.clone(),
                            surface_override,
                            submit_status,
                            approval_state,
                        ),
                        "Approve"
                    }
                }
            }
        }
    }
}

fn submit_permission_response(
    event: TranscriptEvent,
    allow: bool,
    action_endpoint: Option<String>,
    session_id: String,
    client_id: String,
    refresh_context: SurfaceRefreshContext,
    mut surface_override: Signal<Option<WebSurfaceSnapshot>>,
    mut submit_status: Signal<String>,
    mut approval_state: Signal<&'static str>,
) {
    let Some(request_id) = event.request_id.clone() else {
        submit_status.set("permission missing request_id".to_string());
        return;
    };
    let Some(endpoint) = action_endpoint else {
        submit_status.set("no action endpoint".to_string());
        return;
    };
    let action_id = format!(
        "auspex-web-permission-{}-{}",
        request_id,
        if allow { "allow" } else { "deny" }
    );
    let request = crate::omegon_web_contract::respond_permission_action(
        action_id,
        client_id.clone(),
        session_id.clone(),
        request_id,
        allow,
    );
    let principal =
        crate::omegon_web_contract::TrustedPrincipalHeaders::auspex_operator("operator:web")
            .display_name("Web Operator")
            .session_id(session_id)
            .client_id(client_id);
    submit_status.set(if allow { "approving" } else { "denying" }.to_string());
    spawn(async move {
        match crate::omegon_web_contract::post_action_request(&endpoint, &request, Some(&principal))
            .await
        {
            Ok(outcome) => {
                let status = format!("{:?}", outcome.status).to_lowercase();
                let accepted = outcome.error.is_none();
                submit_status.set(outcome.message.unwrap_or(status));
                if accepted {
                    approval_state.set(if allow { "approved" } else { "denied" });
                    if let Some(surfaces_endpoint) = refresh_context.surfaces_endpoint.clone() {
                        match crate::omegon_web_contract::refresh_surfaces_snapshot(
                            &surfaces_endpoint,
                        )
                        .await
                        {
                            Ok(snapshot) => {
                                let refreshed = with_bootstrap_status(
                                    normalize_refreshed_surface(&refresh_context, snapshot),
                                    "ACTION REFRESHED",
                                    "actions",
                                );
                                surface_override.set(Some(refreshed));
                            }
                            Err(error) => submit_status
                                .set(format!("permission accepted · refresh failed: {error}")),
                        }
                    }
                }
            }
            Err(error) => submit_status.set(error.to_string()),
        }
    });
}

#[component]
fn Composer(
    queue_mode: String,
    placeholder: String,
    composer: Signal<String>,
    sent_count: Signal<u32>,
    submit_status: Signal<String>,
    surface_override: Signal<Option<WebSurfaceSnapshot>>,
    refresh_context: SurfaceRefreshContext,
    action_endpoint: Option<String>,
    session_id: String,
    client_id: String,
    on_open: EventHandler<ModalTarget>,
) -> Element {
    let mut composer = composer;
    let mut sent_count = sent_count;
    let mut submit_status = submit_status;
    let mut surface_override = surface_override;
    rsx! {
        section { class: "owm-panel owm-composer-panel",
            div { class: "owm-composer-meta",
                span { "queue mode: {queue_mode}" }
                span { "sent: {sent_count}" }
                span { "action: {submit_status}" }
            }
            textarea {
                value: "{composer}",
                placeholder: "{placeholder}",
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
                        let refresh_context = refresh_context.clone();
                        submit_status.set("posting".to_string());
                        spawn(async move {
                            match crate::omegon_web_contract::post_action_request(&endpoint, &request, Some(&principal)).await {
                                Ok(outcome) => {
                                    let status = format!("{:?}", outcome.status).to_lowercase();
                                    let accepted = outcome.error.is_none();
                                    let outcome_message = outcome.message.unwrap_or(status);
                                    submit_status.set(outcome_message.clone());
                                    if accepted {
                                        let next = *sent_count.read() + 1;
                                        sent_count.set(next);
                                        composer.set(String::new());
                                        if let Some(surfaces_endpoint) = refresh_context.surfaces_endpoint.clone() {
                                            match crate::omegon_web_contract::refresh_surfaces_snapshot(&surfaces_endpoint).await {
                                                Ok(snapshot) => {
                                                    let refreshed = with_bootstrap_status(
                                                        normalize_refreshed_surface(&refresh_context, snapshot),
                                                        "STREAM LIVE",
                                                        "refreshed",
                                                    );
                                                    surface_override.set(Some(refreshed));
                                                    submit_status.set(format!("{outcome_message} · refreshed"));
                                                }
                                                Err(error) => submit_status.set(format!(
                                                    "{outcome_message} · refresh failed: {error}"
                                                )),
                                            }
                                        }
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
fn CommandDock(composer: Signal<String>, on_open: EventHandler<ModalTarget>) -> Element {
    let mut composer_signal = composer;
    rsx! {
        div { class: "owm-command-dock",
            div { class: "owm-command-grid",
                for item in COMMAND_ITEMS.iter() {
                    button {
                        class: "owm-command-item",
                        onclick: move |_| composer_signal.set(item.command.to_string()),
                        span { class: "owm-command-group", "{item.group}" }
                        strong { "{item.command}" }
                        p { "{item.detail}" }
                    }
                }
            }
            div { class: "owm-command-utilities",
                button { class: "owm-ghost-button", onclick: move |_| on_open.call(ModalTarget::Settings), "Settings" }
                button { class: "owm-ghost-button", onclick: move |_| on_open.call(ModalTarget::Config), "Config" }
                button { class: "owm-ghost-button", onclick: move |_| on_open.call(ModalTarget::Armory), "Armory" }
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

#[cfg(target_arch = "wasm32")]
fn stream_websocket_url(endpoint: &str) -> Result<String, String> {
    if endpoint.starts_with("ws://") || endpoint.starts_with("wss://") {
        return Ok(endpoint.to_string());
    }
    if endpoint.starts_with("http://") {
        return Ok(endpoint.replacen("http://", "ws://", 1));
    }
    if endpoint.starts_with("https://") {
        return Ok(endpoint.replacen("https://", "wss://", 1));
    }
    let window = web_sys::window().ok_or_else(|| "window unavailable".to_string())?;
    let location = window.location();
    let protocol = location
        .protocol()
        .map_err(|_| "location.protocol unavailable".to_string())?;
    let host = location
        .host()
        .map_err(|_| "location.host unavailable".to_string())?;
    let scheme = if protocol == "https:" { "wss" } else { "ws" };
    let path = if endpoint.starts_with('/') {
        endpoint.to_string()
    } else {
        format!("/{endpoint}")
    };
    Ok(format!("{scheme}://{host}{path}"))
}

#[cfg(target_arch = "wasm32")]
fn start_surface_stream(
    context: SurfaceRefreshContext,
    mut surface_override: Signal<Option<WebSurfaceSnapshot>>,
    mut stream_status: Signal<String>,
) {
    use wasm_bindgen::JsCast;

    let Some(stream_endpoint) = context.stream_endpoint.clone() else {
        stream_status.set("stream unavailable".to_string());
        return;
    };
    let url = match stream_websocket_url(&stream_endpoint) {
        Ok(url) => crate::omegon_web_contract::endpoint_with_token(
            &url,
            crate::omegon_web_contract::page_query_token().as_deref(),
        ),
        Err(error) => {
            stream_status.set(format!("stream url failed: {error}"));
            return;
        }
    };
    let ws = match web_sys::WebSocket::new(&url) {
        Ok(ws) => ws,
        Err(error) => {
            stream_status.set(format!("stream open failed: {error:?}"));
            return;
        }
    };

    let mut open_status = stream_status;
    let onopen =
        wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_| {
            open_status.set("stream connected".to_string())
        }));
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let message_context = context.clone();
    let mut message_status = stream_status;
    let onmessage = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::MessageEvent)>::wrap(
        Box::new(move |event: web_sys::MessageEvent| {
            let Some(text) = event.data().as_string() else {
                message_status.set("stream non-text frame".to_string());
                return;
            };
            let envelope: crate::omegon_web_contract::BackendSurfaceStreamEnvelope =
                match serde_json::from_str(&text) {
                    Ok(envelope) => envelope,
                    Err(error) => {
                        message_status.set(format!("stream decode failed: {error}"));
                        return;
                    }
                };
            if envelope.event_type == "snapshot" {
                match serde_json::from_value::<crate::omegon_web_contract::BackendSurfacesSnapshot>(
                    envelope.payload,
                ) {
                    Ok(snapshot) => {
                        let refreshed = with_bootstrap_status(
                            normalize_refreshed_surface(&message_context, snapshot),
                            "STREAM LIVE",
                            "stream",
                        );
                        surface_override.set(Some(refreshed));
                        message_status.set(format!("stream snapshot r{}", envelope.revision));
                    }
                    Err(error) => {
                        message_status.set(format!("stream snapshot decode failed: {error}"))
                    }
                }
                return;
            }

            let Some(surfaces_endpoint) = message_context.surfaces_endpoint.clone() else {
                message_status.set(format!(
                    "stream {} r{}",
                    envelope.event_type, envelope.revision
                ));
                return;
            };
            let refresh_context = message_context.clone();
            let event_type = envelope.event_type.clone();
            let revision = envelope.revision;
            let mut override_signal = surface_override;
            let mut status_signal = message_status;
            wasm_bindgen_futures::spawn_local(async move {
                match crate::omegon_web_contract::refresh_surfaces_snapshot(&surfaces_endpoint)
                    .await
                {
                    Ok(snapshot) => {
                        let refreshed = with_bootstrap_status(
                            normalize_refreshed_surface(&refresh_context, snapshot),
                            "STREAM LIVE",
                            "stream",
                        );
                        override_signal.set(Some(refreshed));
                        status_signal.set(format!("stream {event_type} r{revision} · refreshed"));
                    }
                    Err(error) => status_signal.set(format!(
                        "stream {event_type} r{revision} · refresh failed: {error}"
                    )),
                }
            });
        }),
    );
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let mut error_status = stream_status;
    let onerror =
        wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(move |_| {
            error_status.set("stream error".to_string())
        }));
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let mut close_status = stream_status;
    let onclose = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::CloseEvent)>::wrap(Box::new(
        move |event: web_sys::CloseEvent| {
            close_status.set(format!("stream closed {}", event.code()))
        },
    ));
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    std::mem::forget(ws);
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

    let surface_override = use_signal(|| Option::<WebSurfaceSnapshot>::None);
    let surface = {
        if let Some(surface) = surface_override.read().clone() {
            surface
        } else {
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
        }
    };
    let composer = use_signal(String::new);
    let mut modal_target = use_signal(|| Option::<ModalTarget>::None);
    let approval_state = use_signal(|| "pending");
    let sent_count = use_signal(|| 0_u32);
    let submit_status = use_signal(|| "idle".to_string());
    let stream_status = use_signal(|| "connecting".to_string());
    let stream_started = use_signal(|| false);
    let events_override = use_signal(|| Option::<DaemonEventsSurface>::None);
    let mut tool_modal = use_signal(|| Option::<TranscriptEvent>::None);
    let refresh_context = refresh_context_for(&surface);
    let action_endpoint = surface.session.links.actions.clone();
    let session_id = surface.session.session_id.clone();
    let client_id = "auspex-web".to_string();

    #[cfg(target_arch = "wasm32")]
    {
        let refresh_context = refresh_context.clone();
        let surface_override = surface_override;
        let stream_status = stream_status;
        let mut stream_started = stream_started;
        let mut events_override = events_override;
        use_effect(move || {
            if !*stream_started.read() {
                stream_started.set(true);
                start_surface_stream(refresh_context.clone(), surface_override, stream_status);
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(response) =
                        crate::omegon_web_contract::fetch_events_snapshot("/api/events").await
                    {
                        events_override.set(Some(normalize_daemon_events(&response)));
                    }
                });
            }
        });
    }

    let status = surface.runtime.state.clone();

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
                    DaemonCoreCard { runtime: surface.runtime.clone() }
                    BackendStatusCard { backend: surface.backend, action_status: submit_status, stream_status }
                    DaemonEventsCard { events: events_override.read().clone().unwrap_or(surface.daemon_events) }
                }

                section { class: "owm-conversation-column",
                    section { class: "owm-panel owm-hero-panel",
                        div { class: "owm-panel-heading",
                            div {
                                Eyebrow { label: "CURRENT TURN" }
                                h2 { "Single-agent transcript" }
                            }
                            span { class: "owm-panel-meta", "{surface.transcript.len()} events" }
                        }
                        div { class: "owm-transcript-list",
                            if surface.transcript.is_empty() {
                                div { class: "owm-empty", "Transcript idle — no events this turn" }
                            }
                            for event in surface.transcript.iter() {
                                TranscriptEntry {
                                    event: event.clone(),
                                    action_endpoint: action_endpoint.clone(),
                                    session_id: session_id.clone(),
                                    client_id: client_id.clone(),
                                    refresh_context: refresh_context.clone(),
                                    surface_override,
                                    submit_status,
                                    approval_state,
                                    on_expand: move |ev| tool_modal.set(Some(ev)),
                                }
                            }
                        }
                    }

                    CommandDock {
                        composer,
                        on_open: move |target| modal_target.set(Some(target)),
                    }

                    Composer {
                        queue_mode: surface.composer.queue_mode.clone(),
                        placeholder: surface.composer.placeholder.clone(),
                        composer,
                        sent_count,
                        submit_status,
                        surface_override,
                        refresh_context: refresh_context.clone(),
                        action_endpoint: action_endpoint.clone(),
                        session_id: session_id.clone(),
                        client_id: client_id.clone(),
                        on_open: move |target| modal_target.set(Some(target)),
                    }
                }

                aside { class: "owm-cockpit-rail owm-right-rail",
                    PlanCard { plan: surface.plan }
                    OperationsCard { operations: surface.operations }
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
