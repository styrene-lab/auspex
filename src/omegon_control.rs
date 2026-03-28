use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OmegonStateSnapshot {
    #[serde(default)]
    pub design: DesignSnapshot,
    #[serde(default)]
    pub openspec: OpenSpecSnapshot,
    #[serde(default)]
    pub cleave: CleaveSnapshot,
    #[serde(default)]
    pub session: SessionSnapshot,
    #[serde(default)]
    pub harness: Option<HarnessStatusSnapshot>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesignSnapshot {
    #[serde(default)]
    pub focused: Option<FocusedNode>,
    #[serde(default)]
    pub implementing: Vec<NodeBrief>,
    #[serde(default)]
    pub actionable: Vec<NodeBrief>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct FocusedNode {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub open_questions: Vec<String>,
    pub decisions: usize,
    pub children: usize,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct NodeBrief {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenSpecSnapshot {
    #[serde(default)]
    pub total_tasks: usize,
    #[serde(default)]
    pub done_tasks: usize,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CleaveSnapshot {
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub total_children: usize,
    #[serde(default)]
    pub completed: usize,
    #[serde(default)]
    pub failed: usize,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct SessionSnapshot {
    #[serde(default)]
    pub turns: u32,
    #[serde(default)]
    pub tool_calls: u32,
    #[serde(default)]
    pub compactions: u32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HarnessStatusSnapshot {
    pub git_branch: Option<String>,
    #[serde(default)]
    pub git_detached: bool,
    #[serde(default)]
    pub thinking_level: String,
    #[serde(default)]
    pub capability_tier: String,
    #[serde(default)]
    pub providers: Vec<ProviderStatusSnapshot>,
    #[serde(default)]
    pub memory_available: bool,
    #[serde(default)]
    pub cleave_available: bool,
    pub memory_warning: Option<String>,
    #[serde(default)]
    pub active_delegates: Vec<DelegateSummarySnapshot>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct ProviderStatusSnapshot {
    pub name: String,
    #[serde(default)]
    pub authenticated: bool,
    pub auth_method: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct DelegateSummarySnapshot {
    pub task_id: String,
    pub agent_name: String,
    pub status: String,
    #[serde(default)]
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OmegonEvent {
    StateSnapshot { data: OmegonStateSnapshot },
    MessageStart { role: String },
    MessageChunk { text: String },
    MessageEnd,
    SystemNotification { message: String },
    HarnessStatusChanged { status: HarnessStatusSnapshot },
    SessionReset,
    TurnStart { turn: u32 },
    TurnEnd { turn: u32 },
    ToolStart { id: String, name: String },
    ToolEnd { id: String, is_error: bool },
}
