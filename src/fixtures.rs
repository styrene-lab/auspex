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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttributedText {
    pub text: String,
    pub origin: Option<BlockOrigin>,
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
    pub requested_profile: Option<String>,
    pub requested_model: Option<String>,
    pub status: String,
    pub failure_code: Option<String>,
    pub note: Option<String>,
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
    pub available_options: Vec<DispatcherOptionData>,
    pub switch_state: Option<DispatcherSwitchStateData>,
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
}

impl DevScenario {
    pub const ALL: [Self; 6] = [
        Self::Ready,
        Self::Booting,
        Self::Degraded,
        Self::StartupFailure,
        Self::CompatibilityFailure,
        Self::Reconnecting,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Booting => "booting",
            Self::Degraded => "degraded",
            Self::StartupFailure => "startup-failure",
            Self::CompatibilityFailure => "compat-failure",
            Self::Reconnecting => "reconnecting",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Booting => "Booting",
            Self::Degraded => "Degraded",
            Self::StartupFailure => "Startup failure",
            Self::CompatibilityFailure => "Compat failure",
            Self::Reconnecting => "Reconnecting",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSessionSummary {
    pub connection: String,
    pub activity: String,
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
                work: "No focused work item yet".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::Assistant,
                text: "Auspex scaffold ready. Type a prompt to grow the shell from here.".into(),
            }],
            composer: ComposerState::default(),
        }
    }

    pub fn booting_session() -> Self {
        Self {
            shell_state: ShellState::StartingOmegon,
            scenario: DevScenario::Booting,
            summary: HostSessionSummary {
                connection: "Starting bundled runtime".into(),
                activity: "Launching Styrene and Omegon".into(),
                work: "Work state unavailable during startup".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::System,
                text: "Auspex is starting Styrene and Omegon. The conversation shell will become interactive when the host session is ready.".into(),
            }],
            composer: ComposerState::default(),
        }
    }

    pub fn degraded_session() -> Self {
        Self {
            shell_state: ShellState::Degraded,
            scenario: DevScenario::Degraded,
            summary: HostSessionSummary {
                connection: "Connected locally, relay degraded".into(),
                activity: "Continuing with degraded remote connectivity".into(),
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
        }
    }

    pub fn startup_failure_session() -> Self {
        Self {
            shell_state: ShellState::Failed,
            scenario: DevScenario::StartupFailure,
            summary: HostSessionSummary {
                connection: "Embedded Omegon backend unavailable".into(),
                activity: "Startup blocked by embedded backend failure".into(),
                work: "No local session available".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::System,
                text: "Auspex could not start its embedded Omegon backend. Local operation is blocked until the backend startup contract succeeds.".into(),
            }],
            composer: ComposerState::default(),
        }
    }

    pub fn compatibility_failure_session() -> Self {
        Self {
            shell_state: ShellState::Failed,
            scenario: DevScenario::CompatibilityFailure,
            summary: HostSessionSummary {
                connection: "Host incompatible".into(),
                activity: "Startup blocked by compatibility failure".into(),
                work: "No session available".into(),
            },
            messages: vec![ChatMessage {
                role: MessageRole::System,
                text: "Compatibility failure: Auspex expects Omegon control-plane schema 2, but the detected host did not satisfy the declared contract.".into(),
            }],
            composer: ComposerState::default(),
        }
    }

    pub fn reconnecting_session() -> Self {
        Self {
            shell_state: ShellState::CompatibilityChecking,
            scenario: DevScenario::Reconnecting,
            summary: HostSessionSummary {
                connection: "Reconnecting to desktop host".into(),
                activity: "Restoring remote session link".into(),
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
        static EMPTY: TranscriptData = TranscriptData {
            turns: Vec::new(),
            active_turn: None,
            context_tokens: None,
        };
        &EMPTY
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
        WorkData {
            focused_title: Some("Phase 3 — Simple mode MVP".into()),
            focused_status: Some("decided".into()),
            ..Default::default()
        }
    }

    fn session_data(&self) -> SessionData {
        SessionData {
            git_branch: Some("main".into()),
            thinking_level: "medium".into(),
            capability_tier: "victory".into(),
            providers: vec![ProviderInfo {
                name: "Anthropic".into(),
                authenticated: true,
                model: Some("claude-sonnet".into()),
            }],
            memory_available: true,
            cleave_available: true,
            session_turns: 4,
            session_tool_calls: 12,
            ..Default::default()
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
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            text:
                "No engine is attached yet. This scaffold only proves the basic conversation shell."
                    .into(),
        });
        self.summary.activity = "Waiting for attached engine integration".into();
        self.composer.clear();
        true
    }
}
