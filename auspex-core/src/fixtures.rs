use crate::session_model::HostSessionModel;

// ── View-model types ────────────────────────────────────────

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnBlockText {
    pub text: String,
    pub collapsed: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OriginKind {
    Dispatcher,
    Child,
    System,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockOrigin {
    pub kind: OriginKind,
    pub label: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCard {
    pub id: String,
    pub name: String,
    pub args: String,
    pub partial_output: String,
    pub result: Option<String>,
    pub is_error: bool,
    pub origin: Option<BlockOrigin>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemNoticeKind {
    Generic,
    DispatcherSwitch,
    CleaveStart,
    CleaveComplete,
    ChildStatus,
    Failure,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributedText {
    pub text: String,
    pub origin: Option<BlockOrigin>,
    pub notice_kind: Option<SystemNoticeKind>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnBlock {
    Thinking(TurnBlockText),
    Text(AttributedText),
    Tool(ToolCard),
    System(AttributedText),
    Aborted(String),
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub number: u32,
    pub user_prompt: Option<String>,
    pub blocks: Vec<TurnBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TranscriptData {
    pub turns: Vec<Turn>,
    pub active_turn: Option<u32>,
    pub context_tokens: Option<u64>,
}

/// Brief description of a design-tree node for work-state lists.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WorkNode {
    pub id: String,
    pub title: String,
    pub status: String,
}

/// Provider entry for the Session screen.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProviderInfo {
    pub name: String,
    pub authenticated: bool,
    pub auth_method: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DelegateSummaryData {
    pub task_id: String,
    pub agent_name: String,
    pub status: String,
    pub elapsed_ms: u64,
}

/// Snapshot of design-tree state for the Graph power-mode screen.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct GraphData {
    /// All nodes, or the best available subset (implementing + actionable fallback).
    pub nodes: Vec<WorkNode>,
    /// True when `nodes` is a full inventory; false when it's a partial fallback.
    pub is_full_inventory: bool,
    pub counts: Vec<(String, usize)>,
}

/// Snapshot of work state for the Work power-mode screen.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct WorkData {
    pub focused_id: Option<String>,
    pub focused_title: Option<String>,
    pub focused_status: Option<String>,
    /// Number of open questions on the focused node.
    pub open_question_count: usize,
    pub implementing: Vec<WorkNode>,
    pub actionable: Vec<WorkNode>,
    pub openspec_total: usize,
    pub openspec_done: usize,
    pub cleave_active: bool,
    pub cleave_total: usize,
    pub cleave_completed: usize,
    pub cleave_failed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DispatcherOptionData {
    pub profile: String,
    pub label: String,
    pub model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DispatcherSwitchStateData {
    pub request_id: Option<String>,
    pub requested_profile: Option<String>,
    pub requested_model: Option<String>,
    pub status: String,
    pub failure_code: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InstanceIdentityData {
    pub instance_id: String,
    pub role: String,
    pub profile: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InstanceWorkspaceData {
    pub cwd: Option<String>,
    pub workspace_id: Option<String>,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InstanceControlPlaneData {
    pub schema_version: u32,
    pub omegon_version: Option<String>,
    pub base_url: Option<String>,
    pub startup_url: Option<String>,
    pub state_url: Option<String>,
    pub health_url: Option<String>,
    pub ready_url: Option<String>,
    pub ws_url: Option<String>,
    pub auth_mode: Option<String>,
    pub token_ref: Option<String>,
    pub last_ready_at: Option<String>,
    pub last_verified_at: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InstanceRuntimeData {
    pub backend: Option<String>,
    pub host: Option<String>,
    pub pid: Option<u32>,
    pub placement_id: Option<String>,
    pub namespace: Option<String>,
    pub pod_name: Option<String>,
    pub container_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InstanceSessionDescriptorData {
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InstancePolicyData {
    pub model: Option<String>,
    pub thinking_level: Option<String>,
    pub capability_tier: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct InstanceDescriptorData {
    pub identity: InstanceIdentityData,
    pub workspace: Option<InstanceWorkspaceData>,
    pub control_plane: Option<InstanceControlPlaneData>,
    pub runtime: Option<InstanceRuntimeData>,
    pub session: Option<InstanceSessionDescriptorData>,
    pub policy: Option<InstancePolicyData>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DispatcherBindingData {
    pub session_id: String,
    pub dispatcher_instance_id: String,
    pub expected_role: String,
    pub expected_profile: String,
    pub expected_model: Option<String>,
    pub control_plane_schema: u32,
    pub token_ref: Option<String>,
    pub observed_base_url: Option<String>,
    pub last_verified_at: Option<String>,
    pub instance_descriptor: Option<InstanceDescriptorData>,
    pub available_options: Vec<DispatcherOptionData>,
    pub switch_state: Option<DispatcherSwitchStateData>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProviderTelemetryData {
    pub provider: String,
    pub source: String,
    pub route_id: Option<String>,
    pub instance_id: Option<String>,
    pub role: Option<String>,
    pub profile: Option<String>,
    pub model: Option<String>,
    pub requests_remaining: Option<u64>,
    pub tokens_remaining: Option<u64>,
    pub retry_after_secs: Option<u64>,
    pub request_id: Option<String>,
    pub unified_5h_utilization_pct: Option<String>,
    pub unified_7d_utilization_pct: Option<String>,
    pub codex_primary_used_pct: Option<String>,
    pub codex_secondary_used_pct: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ControlPlaneTelemetryData {
    pub route_id: Option<String>,
    pub instance_id: Option<String>,
    pub role: Option<String>,
    pub profile: Option<String>,
    pub startup_url: Option<String>,
    pub health_url: Option<String>,
    pub ready_url: Option<String>,
    pub auth_mode: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LifecycleInstanceTelemetryData {
    pub instance_id: String,
    pub route_id: String,
    pub role: String,
    pub profile: String,
    pub base_url: Option<String>,
    pub status: Option<String>,
    pub freshness: Option<String>,
    pub last_seen_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LifecycleRollupCountsData {
    pub total_attached: usize,
    pub fresh: usize,
    pub stale: usize,
    pub lost: usize,
    pub abandoned: usize,
    pub reaped: usize,
    pub unknown: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LifecycleTelemetryData {
    pub summary: String,
    pub attached_count: usize,
    pub selected_route_id: Option<String>,
    pub selected_instance: Option<LifecycleInstanceTelemetryData>,
    pub counts: LifecycleRollupCountsData,
    pub instances: Vec<LifecycleInstanceTelemetryData>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SessionTelemetryData {
    pub provider_summary: String,
    pub lifecycle_summary: String,
    pub lifecycle: LifecycleTelemetryData,
    pub route_summary: String,
    pub latest_turn_summary: String,
    pub latest_provider_telemetry: Option<ProviderTelemetryData>,
    pub provider_rollups: Vec<ProviderTelemetryData>,
    pub latest_estimated_tokens: Option<u64>,
    pub latest_actual_input_tokens: Option<u64>,
    pub latest_actual_output_tokens: Option<u64>,
    pub latest_cache_read_tokens: Option<u64>,
    pub control_plane: Option<ControlPlaneTelemetryData>,
    pub control_plane_rollups: Vec<ControlPlaneTelemetryData>,
}

/// Snapshot of harness and session state for the Session power-mode screen.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SessionData {
    pub git_branch: Option<String>,
    pub git_detached: bool,
    pub thinking_level: String,
    pub capability_tier: String,
    pub providers: Vec<ProviderInfo>,
    pub memory_available: bool,
    pub cleave_available: bool,
    pub memory_warning: Option<String>,
    pub active_delegate_count: usize,
    pub active_delegates: Vec<DelegateSummaryData>,
    pub session_turns: u32,
    pub session_tool_calls: u32,
    pub session_compactions: u32,
    pub context_tokens: Option<u64>,
    pub context_window: Option<u64>,
    pub telemetry: SessionTelemetryData,
    pub instance_descriptor: Option<InstanceDescriptorData>,
    pub dispatcher_binding: Option<DispatcherBindingData>,
}

// ── Chat types ───────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellState {
    StartingOmegon,
    CompatibilityChecking,
    Ready,
    Degraded,
    Failed,
}

#[allow(dead_code)]
impl ShellState {
    pub fn label(self) -> &'static str {
        match self {
            Self::StartingOmegon => "Starting Omegon",
            Self::CompatibilityChecking => "Checking compatibility",
            Self::Ready => "Ready",
            Self::Degraded => "Degraded",
            Self::Failed => "Failed",
        }
    }

    pub fn status_class(self) -> &'static str {
        match self {
            Self::Ready => "status status-ready",
            Self::Degraded => "status status-degraded",
            Self::Failed => "status status-failed",
            _ => "status status-pending",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevScenario {
    Ready,
    Booting,
    Degraded,
    StartupFailure,
    CompatibilityFailure,
    Reconnecting,
    LocalDevQuiet,
    LocalDevBusy,
    HomelabFleet,
    EnterpriseIncident,
}

impl DevScenario {
    pub fn key(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Booting => "booting",
            Self::Degraded => "degraded",
            Self::StartupFailure => "startup-failure",
            Self::CompatibilityFailure => "compat-failure",
            Self::Reconnecting => "reconnecting",
            Self::LocalDevQuiet => "local-dev-quiet",
            Self::LocalDevBusy => "local-dev-busy",
            Self::HomelabFleet => "homelab-fleet",
            Self::EnterpriseIncident => "enterprise-incident",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityKind {
    Idle,
    Running,
    Waiting,
    Degraded,
    Completed,
    Failure,
}

impl ActivityKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Degraded => "degraded",
            Self::Completed => "completed",
            Self::Failure => "failure",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadinessStepState {
    Pending,
    Active,
    Complete,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessStepData {
    pub label: String,
    pub detail: String,
    pub state: ReadinessStepState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperatorReadinessData {
    pub ready: bool,
    pub title: String,
    pub detail: String,
    pub steps: Vec<ReadinessStepData>,
}

impl Default for OperatorReadinessData {
    fn default() -> Self {
        Self {
            ready: true,
            title: "Ready".into(),
            detail: "Operator controls are ready.".into(),
            steps: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum AppSurfaceKind {
    BootstrapNote,
    Startup,
    Reconnecting,
    StartupFailure,
    CompatibilityFailure,
}

#[allow(dead_code)]
impl AppSurfaceKind {
    pub fn section_class(self) -> &'static str {
        match self {
            Self::BootstrapNote => "bootstrap-note",
            Self::Startup => "state-screen state-screen-starting",
            Self::Reconnecting => "banner banner-reconnecting",
            Self::StartupFailure | Self::CompatibilityFailure => {
                "status-panel status-panel-failure"
            }
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::BootstrapNote => "Bootstrap",
            Self::Startup => "Starting Omegon",
            Self::Reconnecting => "Reconnecting…",
            Self::StartupFailure => "Embedded backend startup failed",
            Self::CompatibilityFailure => "Compatibility failure",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct AppSurfaceNotice {
    pub kind: AppSurfaceKind,
    pub body: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSessionSummary {
    pub connection: String,
    pub activity: String,
    pub activity_kind: ActivityKind,
    pub work: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComposerState {
    draft: String,
}

impl ComposerState {
    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub fn set_draft(&mut self, value: impl Into<String>) {
        self.draft = value.into();
    }

    pub fn clear(&mut self) {
        self.draft.clear();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockHostSession {
    shell_state: ShellState,
    scenario: DevScenario,
    summary: HostSessionSummary,
    messages: Vec<ChatMessage>,
    composer: ComposerState,
    transcript: TranscriptData,
}

impl Default for MockHostSession {
    fn default() -> Self {
        Self::from_scenario(DevScenario::Ready)
    }
}

impl MockHostSession {
    pub fn ready_session() -> Self {
        Self {
            shell_state: ShellState::Ready,
            scenario: DevScenario::Ready,
            summary: HostSessionSummary {
                connection: "Connected to local host session".into(),
                activity: "Idle — waiting for your next prompt".into(),
                activity_kind: ActivityKind::Idle,
                work: "No focused work item yet".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::Assistant,
                text: "Auspex scaffold ready. Type a prompt to grow the shell from here.".into(),
            }],
            composer: ComposerState::default(),
            transcript: TranscriptData::default(),
        }
    }

    pub fn booting_session() -> Self {
        Self {
            shell_state: ShellState::StartingOmegon,
            scenario: DevScenario::Booting,
            summary: HostSessionSummary {
                connection: "Starting bundled runtime".into(),
                activity: "Launching Styrene and Omegon".into(),
                activity_kind: ActivityKind::Waiting,
                work: "Work state unavailable during startup".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::System,
                text: "Auspex is starting Styrene and Omegon. The conversation shell will become interactive when the host session is ready.".into(),
            }],
            composer: ComposerState::default(),
            transcript: TranscriptData::default(),
        }
    }

    pub fn degraded_session() -> Self {
        Self {
            shell_state: ShellState::Degraded,
            scenario: DevScenario::Degraded,
            summary: HostSessionSummary {
                connection: "Connected locally, relay degraded".into(),
                activity: "Continuing with degraded remote connectivity".into(),
                activity_kind: ActivityKind::Degraded,
                work: "1 cached work item in progress".into(),
            },
            messages: vec![
                ChatMessage {
                    role: MessageRole::Assistant,
                    text: "Cached session restored. The local shell is still usable.".into(),
                },
                ChatMessage {
                    role: MessageRole::System,
                    text: "Styrene relay is degraded. Phone clients may reconnect automatically while local work continues.".into(),
                },
            ],
            composer: ComposerState::default(),
            transcript: TranscriptData::default(),
        }
    }

    pub fn startup_failure_session() -> Self {
        Self {
            shell_state: ShellState::Failed,
            scenario: DevScenario::StartupFailure,
            summary: HostSessionSummary {
                connection: "Embedded Omegon backend unavailable".into(),
                activity: "Startup blocked by embedded backend failure".into(),
                activity_kind: ActivityKind::Failure,
                work: "No local session available".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::System,
                text: "Auspex could not start its embedded Omegon backend. Local operation is blocked until the backend startup contract succeeds.".into(),
            }],
            composer: ComposerState::default(),
            transcript: TranscriptData::default(),
        }
    }

    pub fn compatibility_failure_session() -> Self {
        Self {
            shell_state: ShellState::Failed,
            scenario: DevScenario::CompatibilityFailure,
            summary: HostSessionSummary {
                connection: "Host incompatible".into(),
                activity: "Startup blocked by compatibility failure".into(),
                activity_kind: ActivityKind::Failure,
                work: "No session available".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::System,
                text: "Compatibility failure: Auspex expects Omegon control-plane schema 2, but the detected host did not satisfy the declared contract.".into(),
            }],
            composer: ComposerState::default(),
            transcript: TranscriptData::default(),
        }
    }

    pub fn reconnecting_session() -> Self {
        Self {
            shell_state: ShellState::CompatibilityChecking,
            scenario: DevScenario::Reconnecting,
            summary: HostSessionSummary {
                connection: "Reconnecting to desktop host".into(),
                activity: "Restoring remote session link".into(),
                activity_kind: ActivityKind::Waiting,
                work: "Showing last known focused work".into(),
            },
            messages: vec![
                ChatMessage {
                    role: MessageRole::Assistant,
                    text: "Last known session state is still visible.".into(),
                },
                ChatMessage {
                    role: MessageRole::System,
                    text: "Connection to the host is reconnecting. New input is temporarily paused while Auspex restores the session link.".into(),
                },
            ],
            composer: ComposerState::default(),
            transcript: TranscriptData::default(),
        }
    }

    pub fn local_dev_quiet_session() -> Self {
        Self {
            shell_state: ShellState::Ready,
            scenario: DevScenario::LocalDevQuiet,
            summary: HostSessionSummary {
                connection: "Connected to local host session".into(),
                activity: "Idle — waiting for the next local prompt".into(),
                activity_kind: ActivityKind::Idle,
                work: "No focused work item yet".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::Assistant,
                text: "Quiet local-dev fixture loaded. This scenario is optimized for baseline layout checks with minimal operational noise.".into(),
            }],
            composer: ComposerState::default(),
            transcript: TranscriptData::default(),
        }
    }

    pub fn local_dev_busy_session() -> Self {
        let mut composer = ComposerState::default();
        composer.set_draft("Summarize the last dispatcher switch and propose the next smallest implementation step.");
        Self {
            shell_state: ShellState::Degraded,
            scenario: DevScenario::LocalDevBusy,
            summary: HostSessionSummary {
                connection: "Connected to local host session".into(),
                activity: "Input paused pending provider auth".into(),
                activity_kind: ActivityKind::Running,
                work: "Dispatcher posture review in progress".into(),
            },
            messages: vec![
                ChatMessage {
                    role: MessageRole::Assistant,
                    text: "Busy local-dev fixture loaded. The shell shows a blocked composer, active delegates, and recent transcript density.".into(),
                },
                ChatMessage {
                    role: MessageRole::System,
                    text: "Provider auth is incomplete. Sending is paused until a provider is authenticated.".into(),
                },
            ],
            composer,
            transcript: TranscriptData {
                active_turn: Some(5),
                context_tokens: Some(148_000),
                turns: vec![
                    Turn {
                        user_prompt: None,
                        number: 1,
                        blocks: vec![TurnBlock::Text(AttributedText {
                            text: "Summarize the current session, branch, and work focus around primary_driver attached to workspace unknown.".into(),
                            origin: None,
                            notice_kind: None,
                        })],
                    },
                    Turn {
                        user_prompt: None,
                        number: 2,
                        blocks: vec![
                            TurnBlock::System(AttributedText {
                                text: "Dispatcher switch confirmed (dispatcher-switch-17): primary-interactive · anthropic:claude-sonnet-4-6".into(),
                                origin: Some(BlockOrigin {
                                    kind: OriginKind::Dispatcher,
                                    label: "primary_driver".into(),
                                }),
                                notice_kind: Some(SystemNoticeKind::DispatcherSwitch),
                            }),
                            TurnBlock::Text(AttributedText {
                                text: "Current branch is main and the focused work remains the shell integration pass.".into(),
                                origin: Some(BlockOrigin {
                                    kind: OriginKind::Dispatcher,
                                    label: "primary_driver".into(),
                                }),
                                notice_kind: None,
                            }),
                        ],
                    },
                    Turn {
                        user_prompt: None,
                        number: 3,
                        blocks: vec![TurnBlock::Tool(ToolCard {
                            id: "tool-1".into(),
                            name: "cargo test".into(),
                            args: "--lib screens::tests".into(),
                            partial_output: String::new(),
                            result: Some("188 passed; 0 failed".into()),
                            is_error: false,
                            origin: Some(BlockOrigin {
                                kind: OriginKind::Child,
                                label: "subtask-1".into(),
                            }),
                        })],
                    },
                    Turn {
                        user_prompt: None,
                        number: 4,
                        blocks: vec![TurnBlock::System(AttributedText {
                            text: "Prompt execution blocked: authenticate a provider before sending prompts so Auspex can route work to a runnable model backend.".into(),
                            origin: Some(BlockOrigin {
                                kind: OriginKind::System,
                                label: "Auspex".into(),
                            }),
                            notice_kind: Some(SystemNoticeKind::Failure),
                        })],
                    },
                ],
            },
        }
    }

    pub fn homelab_fleet_session() -> Self {
        Self {
            shell_state: ShellState::Ready,
            scenario: DevScenario::HomelabFleet,
            summary: HostSessionSummary {
                connection: "Connected to homelab control plane".into(),
                activity: "Fleet overview with mixed freshness across attached instances".into(),
                activity_kind: ActivityKind::Running,
                work: "4 attached routes · 2 delegates active".into(),
            },
            messages: vec![
                ChatMessage {
                    role: MessageRole::Assistant,
                    text: "Homelab fleet fixture loaded. This scenario stresses right-rail summaries and lifecycle density.".into(),
                },
                ChatMessage {
                    role: MessageRole::System,
                    text: "Two detached services are stale but still report control-plane metadata.".into(),
                },
            ],
            composer: ComposerState::default(),
            transcript: TranscriptData {
                active_turn: Some(8),
                context_tokens: Some(62_000),
                turns: vec![
                    Turn {
                        user_prompt: None,
                        number: 7,
                        blocks: vec![TurnBlock::Text(AttributedText {
                            text: "List all stale detached services and identify any with mismatched profiles.".into(),
                            origin: None,
                            notice_kind: None,
                        })],
                    },
                    Turn {
                        user_prompt: None,
                        number: 8,
                        blocks: vec![
                            TurnBlock::Text(AttributedText {
                                text: "Detached service omg_detached_backup is stale on profile homelab-watch, while omg_media_index remains fresh on profile media-index.".into(),
                                origin: Some(BlockOrigin {
                                    kind: OriginKind::Dispatcher,
                                    label: "primary_driver".into(),
                                }),
                                notice_kind: None,
                            }),
                            TurnBlock::System(AttributedText {
                                text: "Lifecycle rollup: fresh 4 · stale 2 · lost 1".into(),
                                origin: Some(BlockOrigin {
                                    kind: OriginKind::System,
                                    label: "Auspex".into(),
                                }),
                                notice_kind: Some(SystemNoticeKind::Generic),
                            }),
                        ],
                    },
                ],
            },
        }
    }

    pub fn enterprise_incident_session() -> Self {
        let mut composer = ComposerState::default();
        composer.set_draft("Summarize the enterprise incident state and propose a triage sequence.");
        Self {
            shell_state: ShellState::Degraded,
            scenario: DevScenario::EnterpriseIncident,
            summary: HostSessionSummary {
                connection: "Connected to enterprise relay, incident mode".into(),
                activity: "Provider exhaustion and stale routes require triage".into(),
                activity_kind: ActivityKind::Failure,
                work: "12 attached routes · 3 stale · 1 abandoned".into(),
            },
            messages: vec![
                ChatMessage {
                    role: MessageRole::Assistant,
                    text: "Enterprise incident fixture loaded. This pack is intended to stress every shell surface with high-density degraded-state data.".into(),
                },
                ChatMessage {
                    role: MessageRole::System,
                    text: "Anthropic is exhausted for the current 5h window and the primary detached analytics route has entered stale lifecycle state.".into(),
                },
            ],
            composer,
            transcript: TranscriptData {
                active_turn: Some(19),
                context_tokens: Some(182_000),
                turns: vec![
                    Turn {
                        user_prompt: None,
                        number: 18,
                        blocks: vec![TurnBlock::System(AttributedText {
                            text: "Dispatcher switch requested (dispatcher-switch-42): enterprise-triage · openai:gpt-4.1".into(),
                            origin: Some(BlockOrigin {
                                kind: OriginKind::Dispatcher,
                                label: "primary_driver".into(),
                            }),
                            notice_kind: Some(SystemNoticeKind::DispatcherSwitch),
                        })],
                    },
                    Turn {
                        user_prompt: None,
                        number: 19,
                        blocks: vec![
                            TurnBlock::Text(AttributedText {
                                text: "Provider exhaustion detected for anthropic. Remaining enterprise routes should be redistributed to openai:gpt-4.1 and local codex workers until the retry window clears.".into(),
                                origin: Some(BlockOrigin {
                                    kind: OriginKind::Dispatcher,
                                    label: "primary_driver".into(),
                                }),
                                notice_kind: None,
                            }),
                            TurnBlock::System(AttributedText {
                                text: "Abandoned route omg_enterprise_batch_03 exceeded lifecycle policy and awaits reap.".into(),
                                origin: Some(BlockOrigin {
                                    kind: OriginKind::System,
                                    label: "Auspex".into(),
                                }),
                                notice_kind: Some(SystemNoticeKind::Failure),
                            }),
                        ],
                    },
                ],
            },
        }
    }

    pub fn from_scenario(scenario: DevScenario) -> Self {
        match scenario {
            DevScenario::Ready => Self::ready_session(),
            DevScenario::Booting => Self::booting_session(),
            DevScenario::Degraded => Self::degraded_session(),
            DevScenario::StartupFailure => Self::startup_failure_session(),
            DevScenario::CompatibilityFailure => Self::compatibility_failure_session(),
            DevScenario::Reconnecting => Self::reconnecting_session(),
            DevScenario::LocalDevQuiet => Self::local_dev_quiet_session(),
            DevScenario::LocalDevBusy => Self::local_dev_busy_session(),
            DevScenario::HomelabFleet => Self::homelab_fleet_session(),
            DevScenario::EnterpriseIncident => Self::enterprise_incident_session(),
        }
    }
}

impl HostSessionModel for MockHostSession {
    fn shell_state(&self) -> ShellState {
        self.shell_state
    }

    fn scenario(&self) -> DevScenario {
        self.scenario
    }

    fn summary(&self) -> &HostSessionSummary {
        &self.summary
    }

    fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    fn transcript(&self) -> &TranscriptData {
        &self.transcript
    }

    fn composer(&self) -> &ComposerState {
        &self.composer
    }

    fn composer_mut(&mut self) -> &mut ComposerState {
        &mut self.composer
    }

    fn set_scenario(&mut self, scenario: DevScenario) {
        *self = Self::from_scenario(scenario);
    }

    fn can_submit(&self) -> bool {
        self.shell_state == ShellState::Ready || self.shell_state == ShellState::Degraded
    }

    fn is_run_active(&self) -> bool {
        false
    }

    fn work_data(&self) -> WorkData {
        match self.scenario {
            DevScenario::LocalDevQuiet => WorkData {
                focused_id: Some("simple-mode-mvp".into()),
                focused_title: Some("Phase 3 — Simple mode MVP".into()),
                focused_status: Some("decided".into()),
                open_question_count: 0,
                actionable: vec![WorkNode {
                    id: "simple-mode-shell".into(),
                    title: "Wire shell composition for local dev".into(),
                    status: "ready".into(),
                }],
                openspec_total: 3,
                openspec_done: 2,
                ..Default::default()
            },
            DevScenario::LocalDevBusy => WorkData {
                focused_id: Some("shell-integration".into()),
                focused_title: Some("Dispatcher posture review".into()),
                focused_status: Some("implementing".into()),
                open_question_count: 2,
                implementing: vec![WorkNode {
                    id: "embedded-bootstrap".into(),
                    title: "Repair embedded control-plane bootstrap".into(),
                    status: "implementing".into(),
                }],
                actionable: vec![
                    WorkNode {
                        id: "provider-auth".into(),
                        title: "Authenticate a runnable provider".into(),
                        status: "blocked".into(),
                    },
                    WorkNode {
                        id: "dispatcher-review".into(),
                        title: "Confirm dispatcher route and profile".into(),
                        status: "ready".into(),
                    },
                ],
                openspec_total: 5,
                openspec_done: 3,
                cleave_active: true,
                cleave_total: 3,
                cleave_completed: 1,
                ..Default::default()
            },
            DevScenario::HomelabFleet => WorkData {
                focused_id: Some("fleet-overview".into()),
                focused_title: Some("Homelab fleet lifecycle audit".into()),
                focused_status: Some("implementing".into()),
                open_question_count: 1,
                implementing: vec![
                    WorkNode {
                        id: "fleet-lifecycle".into(),
                        title: "Normalize stale route lifecycle summaries".into(),
                        status: "implementing".into(),
                    },
                    WorkNode {
                        id: "control-plane-card".into(),
                        title: "Surface detached instance control-plane metadata".into(),
                        status: "implementing".into(),
                    },
                ],
                actionable: vec![WorkNode {
                    id: "profile-drift".into(),
                    title: "Investigate mismatched detached profiles".into(),
                    status: "ready".into(),
                }],
                openspec_total: 7,
                openspec_done: 4,
                cleave_active: true,
                cleave_total: 2,
                cleave_completed: 1,
                ..Default::default()
            },
            DevScenario::EnterpriseIncident => WorkData {
                focused_id: Some("enterprise-triage".into()),
                focused_title: Some("Enterprise incident triage".into()),
                focused_status: Some("implementing".into()),
                open_question_count: 4,
                implementing: vec![
                    WorkNode {
                        id: "provider-failover".into(),
                        title: "Redistribute exhausted provider load".into(),
                        status: "implementing".into(),
                    },
                    WorkNode {
                        id: "stale-routes".into(),
                        title: "Triage stale and abandoned enterprise routes".into(),
                        status: "implementing".into(),
                    },
                ],
                actionable: vec![
                    WorkNode {
                        id: "incident-summary".into(),
                        title: "Produce incident commander summary".into(),
                        status: "ready".into(),
                    },
                    WorkNode {
                        id: "reap-policy".into(),
                        title: "Confirm reap policy for abandoned route".into(),
                        status: "blocked".into(),
                    },
                ],
                openspec_total: 9,
                openspec_done: 5,
                cleave_active: true,
                cleave_total: 4,
                cleave_completed: 2,
                cleave_failed: 1,
            },
            _ => WorkData {
                focused_title: Some("Phase 3 — Simple mode MVP".into()),
                focused_status: Some("decided".into()),
                ..Default::default()
            },
        }
    }

    fn session_data(&self) -> SessionData {
        match self.scenario {
            DevScenario::LocalDevQuiet => SessionData {
                git_branch: Some("main".into()),
                thinking_level: "medium".into(),
                capability_tier: "victory".into(),
                providers: vec![ProviderInfo {
                    name: "Anthropic".into(),
                    authenticated: true,
                    auth_method: Some("oauth".into()),
                    model: Some("claude-sonnet".into()),
                }],
                memory_available: true,
                cleave_available: true,
                session_turns: 1,
                session_tool_calls: 0,
                telemetry: SessionTelemetryData {
                    provider_summary: "1 / 1 authenticated".into(),
                    lifecycle_summary: "no active delegates".into(),
                    lifecycle: LifecycleTelemetryData {
                        summary: "single local shell attached".into(),
                        ..Default::default()
                    },
                    route_summary: "local shell".into(),
                    latest_turn_summary: "turns 1 · tool calls 0".into(),
                    latest_provider_telemetry: None,
                    provider_rollups: Vec::new(),
                    latest_estimated_tokens: Some(2_400),
                    latest_actual_input_tokens: None,
                    latest_actual_output_tokens: None,
                    latest_cache_read_tokens: None,
                    control_plane: None,
                    control_plane_rollups: Vec::new(),
                },
                ..Default::default()
            },
            DevScenario::LocalDevBusy => SessionData {
                git_branch: Some("main".into()),
                thinking_level: "high".into(),
                capability_tier: "victory".into(),
                providers: vec![
                    ProviderInfo {
                        name: "Anthropic".into(),
                        authenticated: false,
                        auth_method: Some("oauth".into()),
                        model: Some("claude-sonnet-4-6".into()),
                    },
                    ProviderInfo {
                        name: "OpenAI".into(),
                        authenticated: false,
                        auth_method: Some("api key".into()),
                        model: Some("gpt-5-codex".into()),
                    },
                ],
                memory_available: true,
                cleave_available: true,
                active_delegate_count: 1,
                active_delegates: vec![DelegateSummaryData {
                    task_id: "subtask-1".into(),
                    agent_name: "analyzer".into(),
                    status: "running".into(),
                    elapsed_ms: 84_000,
                }],
                session_turns: 4,
                session_tool_calls: 12,
                context_tokens: Some(148_000),
                context_window: Some(272_000),
                telemetry: SessionTelemetryData {
                    provider_summary: "0 / 2 authenticated".into(),
                    lifecycle_summary: "1 delegate active".into(),
                    lifecycle: LifecycleTelemetryData {
                        summary: "dispatcher attached · 1 active delegate".into(),
                        attached_count: 1,
                        counts: LifecycleRollupCountsData {
                            total_attached: 1,
                            fresh: 1,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    route_summary: "local shell · dispatcher pending auth".into(),
                    latest_turn_summary: "turns 4 · tool calls 12".into(),
                    latest_provider_telemetry: None,
                    provider_rollups: Vec::new(),
                    latest_estimated_tokens: Some(148_000),
                    latest_actual_input_tokens: Some(4_822),
                    latest_actual_output_tokens: Some(1_188),
                    latest_cache_read_tokens: Some(0),
                    control_plane: None,
                    control_plane_rollups: Vec::new(),
                },
                dispatcher_binding: Some(DispatcherBindingData {
                    session_id: "session_01HVLOCAL".into(),
                    dispatcher_instance_id: "primary_driver".into(),
                    expected_role: "dispatcher".into(),
                    expected_profile: "primary-interactive".into(),
                    expected_model: Some("anthropic:claude-sonnet-4-6".into()),
                    control_plane_schema: 1,
                    ..Default::default()
                }),
                ..Default::default()
            },
            DevScenario::HomelabFleet => SessionData {
                git_branch: Some("homelab/fleet-audit".into()),
                thinking_level: "medium".into(),
                capability_tier: "victory".into(),
                providers: vec![
                    ProviderInfo {
                        name: "Anthropic".into(),
                        authenticated: true,
                        auth_method: Some("oauth".into()),
                        model: Some("claude-sonnet-4-6".into()),
                    },
                    ProviderInfo {
                        name: "OpenAI".into(),
                        authenticated: true,
                        auth_method: Some("api key".into()),
                        model: Some("gpt-5-codex".into()),
                    },
                ],
                memory_available: true,
                cleave_available: true,
                active_delegate_count: 2,
                active_delegates: vec![
                    DelegateSummaryData {
                        task_id: "media-index-audit".into(),
                        agent_name: "general".into(),
                        status: "running".into(),
                        elapsed_ms: 212_000,
                    },
                    DelegateSummaryData {
                        task_id: "backup-watch".into(),
                        agent_name: "analyzer".into(),
                        status: "waiting".into(),
                        elapsed_ms: 98_000,
                    },
                ],
                session_turns: 8,
                session_tool_calls: 26,
                context_tokens: Some(62_000),
                context_window: Some(272_000),
                telemetry: SessionTelemetryData {
                    provider_summary: "2 / 2 authenticated".into(),
                    lifecycle_summary: "4 fresh · 2 stale · 1 lost".into(),
                    lifecycle: LifecycleTelemetryData {
                        summary: "7 attached homelab routes".into(),
                        attached_count: 7,
                        counts: LifecycleRollupCountsData {
                            total_attached: 7,
                            fresh: 4,
                            stale: 2,
                            lost: 1,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    route_summary: "homelab control plane".into(),
                    latest_turn_summary: "turns 8 · tool calls 26".into(),
                    latest_provider_telemetry: None,
                    provider_rollups: Vec::new(),
                    latest_estimated_tokens: Some(62_000),
                    latest_actual_input_tokens: Some(2_704),
                    latest_actual_output_tokens: Some(1_006),
                    latest_cache_read_tokens: Some(320),
                    control_plane: None,
                    control_plane_rollups: Vec::new(),
                },
                dispatcher_binding: Some(DispatcherBindingData {
                    session_id: "session_01HVHOME".into(),
                    dispatcher_instance_id: "primary_driver".into(),
                    expected_role: "dispatcher".into(),
                    expected_profile: "homelab-watch".into(),
                    expected_model: Some("anthropic:claude-sonnet-4-6".into()),
                    control_plane_schema: 1,
                    ..Default::default()
                }),
                ..Default::default()
            },
            DevScenario::EnterpriseIncident => SessionData {
                git_branch: Some("incident/enterprise-relay".into()),
                thinking_level: "high".into(),
                capability_tier: "gloriana".into(),
                providers: vec![
                    ProviderInfo {
                        name: "Anthropic".into(),
                        authenticated: true,
                        auth_method: Some("oauth".into()),
                        model: Some("claude-sonnet-4-6".into()),
                    },
                    ProviderInfo {
                        name: "OpenAI".into(),
                        authenticated: true,
                        auth_method: Some("api key".into()),
                        model: Some("gpt-4.1".into()),
                    },
                    ProviderInfo {
                        name: "Local Codex".into(),
                        authenticated: true,
                        auth_method: Some("loopback".into()),
                        model: Some("gpt-5-codex".into()),
                    },
                ],
                memory_available: true,
                cleave_available: true,
                active_delegate_count: 3,
                active_delegates: vec![
                    DelegateSummaryData {
                        task_id: "reroute-primary".into(),
                        agent_name: "general".into(),
                        status: "running".into(),
                        elapsed_ms: 441_000,
                    },
                    DelegateSummaryData {
                        task_id: "quota-audit".into(),
                        agent_name: "analyzer".into(),
                        status: "running".into(),
                        elapsed_ms: 305_000,
                    },
                    DelegateSummaryData {
                        task_id: "reap-abandoned".into(),
                        agent_name: "general".into(),
                        status: "blocked".into(),
                        elapsed_ms: 127_000,
                    },
                ],
                session_turns: 19,
                session_tool_calls: 73,
                session_compactions: 2,
                context_tokens: Some(182_000),
                context_window: Some(272_000),
                telemetry: SessionTelemetryData {
                    provider_summary: "3 providers available · anthropic exhausted".into(),
                    lifecycle_summary: "12 attached · 3 stale · 1 abandoned".into(),
                    lifecycle: LifecycleTelemetryData {
                        summary: "enterprise relay under incident load".into(),
                        attached_count: 12,
                        counts: LifecycleRollupCountsData {
                            total_attached: 12,
                            fresh: 8,
                            stale: 3,
                            abandoned: 1,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    route_summary: "enterprise relay".into(),
                    latest_turn_summary: "turns 19 · tool calls 73".into(),
                    latest_provider_telemetry: Some(ProviderTelemetryData {
                        provider: "Anthropic".into(),
                        source: "dispatcher".into(),
                        profile: Some("enterprise-triage".into()),
                        model: Some("claude-sonnet-4-6".into()),
                        retry_after_secs: Some(2_400),
                        ..Default::default()
                    }),
                    provider_rollups: Vec::new(),
                    latest_estimated_tokens: Some(182_000),
                    latest_actual_input_tokens: Some(9_640),
                    latest_actual_output_tokens: Some(2_118),
                    latest_cache_read_tokens: Some(2_048),
                    control_plane: None,
                    control_plane_rollups: Vec::new(),
                },
                dispatcher_binding: Some(DispatcherBindingData {
                    session_id: "session_01HVENT".into(),
                    dispatcher_instance_id: "primary_driver".into(),
                    expected_role: "dispatcher".into(),
                    expected_profile: "enterprise-triage".into(),
                    expected_model: Some("openai:gpt-4.1".into()),
                    control_plane_schema: 1,
                    ..Default::default()
                }),
                ..Default::default()
            },
            _ => SessionData {
                git_branch: Some("main".into()),
                thinking_level: "medium".into(),
                capability_tier: "victory".into(),
                providers: vec![ProviderInfo {
                    name: "Anthropic".into(),
                    authenticated: true,
                    auth_method: Some("oauth".into()),
                    model: Some("claude-sonnet".into()),
                }],
                memory_available: true,
                cleave_available: true,
                session_turns: 4,
                session_tool_calls: 12,
                telemetry: SessionTelemetryData {
                    provider_summary: "1 / 1 authenticated".into(),
                    lifecycle_summary: "no active delegates".into(),
                    lifecycle: LifecycleTelemetryData {
                        summary: "no attached instances".into(),
                        ..Default::default()
                    },
                    route_summary: "local shell".into(),
                    latest_turn_summary: "turns 4 · tool calls 12".into(),
                    latest_provider_telemetry: None,
                    provider_rollups: Vec::new(),
                    latest_estimated_tokens: None,
                    latest_actual_input_tokens: None,
                    latest_actual_output_tokens: None,
                    latest_cache_read_tokens: None,
                    control_plane: None,
                    control_plane_rollups: Vec::new(),
                },
                ..Default::default()
            },
        }
    }

    fn graph_data(&self) -> GraphData {
        GraphData {
            nodes: vec![
                WorkNode {
                    id: "auspex-mvp".into(),
                    title: "Auspex MVP".into(),
                    status: "implementing".into(),
                },
                WorkNode {
                    id: "simple-mode".into(),
                    title: "Phase 3 — Simple mode".into(),
                    status: "decided".into(),
                },
                WorkNode {
                    id: "power-mode".into(),
                    title: "Phase 4 — Power mode".into(),
                    status: "implementing".into(),
                },
                WorkNode {
                    id: "phone-client".into(),
                    title: "Phase 5 — Phone".into(),
                    status: "seed".into(),
                },
            ],
            is_full_inventory: true,
            counts: vec![
                ("implementing".into(), 2),
                ("decided".into(), 1),
                ("seed".into(), 1),
            ],
        }
    }

    fn submit(&mut self) -> bool {
        if !self.can_submit() {
            return false;
        }

        let trimmed = self.composer.draft().trim();
        if trimmed.is_empty() {
            return false;
        }

        self.messages.push(ChatMessage {
            role: MessageRole::User,
            text: trimmed.to_string(),
        });
        let assistant =
            "No engine is attached yet. This scaffold only proves the basic conversation shell."
                .to_string();
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            text: assistant.clone(),
        });
        let turn_number = self.transcript.turns.len() as u32 + 1;
        self.transcript.turns.push(Turn {
            number: turn_number,
            user_prompt: None,
            blocks: vec![
                TurnBlock::Text(AttributedText {
                    text: trimmed.to_string(),
                    origin: None,
                    notice_kind: None,
                }),
                TurnBlock::Text(AttributedText {
                    text: assistant,
                    origin: Some(BlockOrigin {
                        kind: OriginKind::System,
                        label: "Auspex".into(),
                    }),
                    notice_kind: None,
                }),
            ],
        });
        self.summary.activity = "Waiting for attached engine integration".into();
        self.composer.clear();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{DevScenario, HostSessionModel, MockHostSession};

    #[test]
    fn fixture_packs_have_distinct_density_profiles() {
        let quiet = MockHostSession::from_scenario(DevScenario::LocalDevQuiet);
        let busy = MockHostSession::from_scenario(DevScenario::LocalDevBusy);
        let homelab = MockHostSession::from_scenario(DevScenario::HomelabFleet);
        let enterprise = MockHostSession::from_scenario(DevScenario::EnterpriseIncident);

        assert!(quiet.messages().len() < busy.messages().len());
        assert!(busy.transcript().turns.len() >= 4);
        assert!(homelab.transcript().turns.len() >= 2);
        assert!(enterprise.transcript().context_tokens.unwrap_or_default()
            > homelab.transcript().context_tokens.unwrap_or_default());
    }

    #[test]
    fn fixture_packs_expose_distinct_work_payloads() {
        let quiet = MockHostSession::from_scenario(DevScenario::LocalDevQuiet);
        let busy = MockHostSession::from_scenario(DevScenario::LocalDevBusy);
        let homelab = MockHostSession::from_scenario(DevScenario::HomelabFleet);
        let enterprise = MockHostSession::from_scenario(DevScenario::EnterpriseIncident);

        assert_eq!(
            quiet.work_data().focused_title.as_deref(),
            Some("Phase 3 — Simple mode MVP")
        );
        assert_eq!(
            busy.work_data().focused_title.as_deref(),
            Some("Dispatcher posture review")
        );
        assert_eq!(
            homelab.work_data().focused_title.as_deref(),
            Some("Homelab fleet lifecycle audit")
        );
        assert_eq!(
            enterprise.work_data().focused_title.as_deref(),
            Some("Enterprise incident triage")
        );
        assert!(enterprise.work_data().cleave_failed > 0);
    }

    #[test]
    fn fixture_packs_expose_distinct_session_payloads() {
        let quiet = MockHostSession::from_scenario(DevScenario::LocalDevQuiet);
        let busy = MockHostSession::from_scenario(DevScenario::LocalDevBusy);
        let homelab = MockHostSession::from_scenario(DevScenario::HomelabFleet);
        let enterprise = MockHostSession::from_scenario(DevScenario::EnterpriseIncident);

        assert_eq!(quiet.session_data().providers.len(), 1);
        assert_eq!(busy.session_data().providers.len(), 2);
        assert_eq!(homelab.session_data().active_delegate_count, 2);
        assert_eq!(enterprise.session_data().active_delegate_count, 3);
        assert_eq!(
            busy.session_data()
                .dispatcher_binding
                .as_ref()
                .map(|binding| binding.expected_profile.as_str()),
            Some("primary-interactive")
        );
        assert_eq!(
            enterprise.session_data()
                .dispatcher_binding
                .as_ref()
                .map(|binding| binding.expected_model.as_deref()),
            Some(Some("openai:gpt-4.1"))
        );
        assert_eq!(
            enterprise
                .session_data()
                .telemetry
                .lifecycle
                .counts
                .abandoned,
            1
        );
    }
}
