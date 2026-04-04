use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OmegonStartupInfo {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub addr: String,
    #[serde(default)]
    pub http_base: String,
    #[serde(default)]
    pub state_url: String,
    #[serde(default)]
    pub startup_url: String,
    #[serde(default)]
    pub health_url: String,
    #[serde(default)]
    pub ready_url: String,
    #[serde(default)]
    pub ws_url: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub auth_mode: String,
    #[serde(default)]
    pub auth_source: String,
    #[serde(default)]
    pub control_plane_state: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
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
    #[serde(default)]
    pub dispatcher: Option<DispatcherBindingSnapshot>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct DesignSnapshot {
    #[serde(default)]
    pub focused: Option<FocusedNode>,
    #[serde(default)]
    pub implementing: Vec<NodeBrief>,
    #[serde(default)]
    pub actionable: Vec<NodeBrief>,
    /// Full node inventory from /api/graph (omitted in lightweight snapshots).
    #[serde(default)]
    pub all_nodes: Vec<NodeBrief>,
    /// Aggregate counts keyed by status string (e.g. "implementing": 3).
    #[serde(default)]
    pub counts: std::collections::HashMap<String, usize>,
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
pub struct OpenSpecSnapshot {
    #[serde(default)]
    pub total_tasks: usize,
    #[serde(default)]
    pub done_tasks: usize,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
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
pub struct DispatcherBindingSnapshot {
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub dispatcher_instance_id: String,
    #[serde(default)]
    pub expected_role: String,
    #[serde(default)]
    pub expected_profile: String,
    pub expected_model: Option<String>,
    #[serde(default)]
    pub control_plane_schema: u32,
    pub token_ref: Option<String>,
    pub observed_base_url: Option<String>,
    pub last_verified_at: Option<String>,
    #[serde(default)]
    pub available_options: Vec<DispatcherOptionSnapshot>,
    #[serde(default)]
    pub switch_state: Option<DispatcherSwitchStateSnapshot>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct DispatcherOptionSnapshot {
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub label: String,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct DispatcherSwitchStateSnapshot {
    pub requested_profile: Option<String>,
    pub requested_model: Option<String>,
    #[serde(default)]
    pub status: String,
    pub note: Option<String>,
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
    StateSnapshot {
        data: Box<OmegonStateSnapshot>,
    },
    MessageStart {
        role: String,
    },
    MessageChunk {
        text: String,
    },
    ThinkingChunk {
        text: String,
    },
    MessageEnd,
    MessageAbort,
    SystemNotification {
        message: String,
    },
    HarnessStatusChanged {
        status: HarnessStatusSnapshot,
    },
    SessionReset,
    TurnStart {
        turn: u32,
    },
    TurnEnd {
        turn: u32,
    },
    ToolStart {
        id: String,
        name: String,
        args: Option<serde_json::Value>,
    },
    ToolUpdate {
        id: String,
        partial: String,
    },
    ToolEnd {
        id: String,
        is_error: bool,
        result: Option<String>,
    },
    AgentEnd,
    PhaseChanged {
        phase: String,
    },
    ContextUpdated {
        tokens: u64,
    },
    DecompositionStarted {
        children: Vec<String>,
    },
    DecompositionChildCompleted {
        label: String,
        success: bool,
    },
    DecompositionCompleted {
        merged: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Actual /api/startup payload from `omegon embedded` v0.15.7.
    const REAL_STARTUP_JSON: &str = r#"{
        "schema_version": 2,
        "addr": "127.0.0.1:7842",
        "http_base": "http://127.0.0.1:7842",
        "state_url": "http://127.0.0.1:7842/api/state",
        "startup_url": "http://127.0.0.1:7842/api/startup",
        "health_url": "http://127.0.0.1:7842/api/healthz",
        "ready_url": "http://127.0.0.1:7842/api/readyz",
        "ws_url": "ws://127.0.0.1:7842/ws?token=18a2d7bb93cce660131e9",
        "token": "18a2d7bb93cce660131e9",
        "auth_mode": "ephemeral-bearer",
        "auth_source": "generated",
        "control_plane_state": "ready"
    }"#;

    /// Actual /api/state payload from `omegon embedded` v0.15.7 (empty session).
    const REAL_STATE_JSON: &str = r#"{
        "design": {
            "counts": {
                "total": 0, "seed": 0, "exploring": 0, "resolved": 0,
                "decided": 0, "implementing": 0, "implemented": 0,
                "blocked": 0, "deferred": 0, "open_questions": 0
            },
            "focused": null,
            "implementing": [],
            "actionable": [],
            "all_nodes": []
        },
        "openspec": {"changes": [], "total_tasks": 0, "done_tasks": 0},
        "cleave": {"active": false, "total_children": 0, "completed": 0, "failed": 0, "children": []},
        "session": {"turns": 0, "tool_calls": 0, "compactions": 0}
    }"#;

    /// Actual stdout startup line from `omegon embedded` v0.15.7.
    const REAL_STDOUT_STARTUP_JSON: &str = r#"{
        "type": "omegon.startup",
        "schema_version": 2,
        "pid": 78313,
        "http_base": "http://127.0.0.1:7842",
        "startup_url": "http://127.0.0.1:7842/api/startup",
        "health_url": "http://127.0.0.1:7842/api/healthz",
        "ready_url": "http://127.0.0.1:7842/api/readyz",
        "ws_url": "ws://127.0.0.1:7842/ws?token=18a2d7bb93cce660131e9",
        "auth_mode": "ephemeral-bearer",
        "auth_source": "generated"
    }"#;

    #[test]
    fn deserialize_real_startup_payload() {
        let info: OmegonStartupInfo = serde_json::from_str(REAL_STARTUP_JSON).unwrap();

        assert_eq!(info.schema_version, 2);
        assert_eq!(info.addr, "127.0.0.1:7842");
        assert_eq!(info.http_base, "http://127.0.0.1:7842");
        assert_eq!(info.state_url, "http://127.0.0.1:7842/api/state");
        assert_eq!(info.startup_url, "http://127.0.0.1:7842/api/startup");
        assert_eq!(info.health_url, "http://127.0.0.1:7842/api/healthz");
        assert_eq!(info.ready_url, "http://127.0.0.1:7842/api/readyz");
        assert!(info.ws_url.contains("token="));
        assert_eq!(info.token, "18a2d7bb93cce660131e9");
        assert_eq!(info.auth_mode, "ephemeral-bearer");
        assert_eq!(info.auth_source, "generated");
        assert_eq!(info.control_plane_state, "ready");
    }

    #[test]
    fn deserialize_real_state_payload() {
        let snapshot: OmegonStateSnapshot = serde_json::from_str(REAL_STATE_JSON).unwrap();

        assert_eq!(snapshot.session.turns, 0);
        assert_eq!(snapshot.session.tool_calls, 0);
        assert_eq!(snapshot.openspec.total_tasks, 0);
        assert!(!snapshot.cleave.active);
        assert!(snapshot.design.focused.is_none());
        assert!(snapshot.design.all_nodes.is_empty());
        assert!(snapshot.harness.is_none());
    }

    #[test]
    fn deserialize_real_stdout_startup_line() {
        // The stdout line has extra fields (type, pid) that OmegonStartupInfo
        // doesn't model — serde should ignore them with #[serde(default)].
        let info: OmegonStartupInfo = serde_json::from_str(REAL_STDOUT_STARTUP_JSON).unwrap();

        assert_eq!(info.schema_version, 2);
        assert_eq!(info.http_base, "http://127.0.0.1:7842");
        assert_eq!(info.auth_mode, "ephemeral-bearer");
        // token is not in the stdout line — should default to empty
        assert_eq!(info.token, "");
    }

    #[test]
    fn state_with_counts_deserializes_correctly() {
        let json = r#"{
            "design": {
                "counts": {"total": 5, "implementing": 2, "seed": 3},
                "focused": null,
                "implementing": [],
                "actionable": [],
                "all_nodes": []
            },
            "openspec": {"total_tasks": 10, "done_tasks": 7},
            "cleave": {"active": false, "total_children": 0, "completed": 0, "failed": 0},
            "session": {"turns": 3, "tool_calls": 15, "compactions": 0}
        }"#;

        let snapshot: OmegonStateSnapshot = serde_json::from_str(json).unwrap();

        assert_eq!(snapshot.design.counts.get("total"), Some(&5));
        assert_eq!(snapshot.design.counts.get("implementing"), Some(&2));
        assert_eq!(snapshot.openspec.total_tasks, 10);
        assert_eq!(snapshot.openspec.done_tasks, 7);
        assert_eq!(snapshot.session.turns, 3);
        assert_eq!(snapshot.session.tool_calls, 15);
    }
}
