use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OmegonInstanceDescriptor {
    #[serde(default)]
    pub identity: OmegonInstanceIdentity,
    #[serde(default)]
    pub workspace: Option<OmegonWorkspaceDescriptor>,
    #[serde(default)]
    pub control_plane: Option<OmegonControlPlaneDescriptor>,
    #[serde(default)]
    pub runtime: Option<OmegonRuntimeDescriptor>,
    #[serde(default)]
    pub session: Option<OmegonSessionDescriptor>,
    #[serde(default)]
    pub policy: Option<OmegonPolicyDescriptor>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OmegonInstanceIdentity {
    #[serde(default)]
    pub instance_id: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OmegonWorkspaceDescriptor {
    pub cwd: Option<String>,
    pub workspace_id: Option<String>,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OmegonControlPlaneDescriptor {
    #[serde(default)]
    pub schema_version: u32,
    pub omegon_version: Option<String>,
    #[serde(alias = "http_base")]
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
    pub ipc_socket_path: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OmegonRuntimeDescriptor {
    pub backend: Option<String>,
    pub host: Option<String>,
    pub pid: Option<u32>,
    pub placement_id: Option<String>,
    pub namespace: Option<String>,
    pub pod_name: Option<String>,
    pub container_name: Option<String>,
    pub health: Option<String>,
    #[serde(default)]
    pub provider_ok: bool,
    #[serde(default)]
    pub memory_ok: bool,
    #[serde(default)]
    pub cleave_available: bool,
    pub context_class: Option<String>,
    pub thinking_level: Option<String>,
    pub capability_tier: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OmegonSessionDescriptor {
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct OmegonPolicyDescriptor {
    pub model: Option<String>,
    pub thinking_level: Option<String>,
    pub capability_tier: Option<String>,
}

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
    #[serde(default)]
    pub instance_descriptor: Option<OmegonInstanceDescriptor>,
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
    #[serde(default, alias = "instance")]
    pub instance_descriptor: Option<OmegonInstanceDescriptor>,
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
    #[serde(default)]
    pub reported_active_delegate_count: Option<usize>,
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
    pub instance_descriptor: Option<OmegonInstanceDescriptor>,
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
    pub request_id: Option<String>,
    pub requested_profile: Option<String>,
    pub requested_model: Option<String>,
    #[serde(default)]
    pub status: String,
    pub failure_code: Option<String>,
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

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ProviderTelemetrySnapshot {
    pub provider: String,
    pub source: String,
    pub unified_5h_utilization_pct: Option<f32>,
    pub unified_7d_utilization_pct: Option<f32>,
    pub requests_remaining: Option<u64>,
    pub tokens_remaining: Option<u64>,
    pub retry_after_secs: Option<u64>,
    pub request_id: Option<String>,
    pub codex_active_limit: Option<String>,
    pub codex_primary_pct: Option<u64>,
    pub codex_primary_reset_secs: Option<u64>,
    pub codex_secondary_reset_secs: Option<u64>,
    pub codex_credits_unlimited: Option<bool>,
    pub codex_limit_name: Option<String>,
}

impl Eq for ProviderTelemetrySnapshot {}

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
        #[serde(default)]
        estimated_tokens: Option<u64>,
        #[serde(default)]
        actual_input_tokens: Option<u64>,
        #[serde(default)]
        actual_output_tokens: Option<u64>,
        #[serde(default)]
        cache_read_tokens: Option<u64>,
        #[serde(default)]
        provider_telemetry: Option<ProviderTelemetrySnapshot>,
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
        #[serde(default)]
        context_window: Option<u64>,
        #[serde(default)]
        context_class: Option<String>,
        #[serde(default)]
        thinking_level: Option<String>,
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
    fn deserialize_startup_payload_with_instance_descriptor() {
        let json = r#"{
            "schema_version": 2,
            "http_base": "http://127.0.0.1:7842",
            "state_url": "http://127.0.0.1:7842/api/state",
            "ws_url": "ws://127.0.0.1:7842/ws?token=test",
            "auth_mode": "ephemeral-bearer",
            "auth_source": "generated",
            "instance_descriptor": {
                "identity": {
                    "instance_id": "omg_primary_01HVK6",
                    "role": "primary-driver",
                    "profile": "supervisor-heavy",
                    "status": "ready"
                },
                "workspace": {
                    "cwd": "/repo",
                    "workspace_id": "repo:8f2f4c1",
                    "branch": "main"
                },
                "control_plane": {
                    "schema_version": 2,
                    "omegon_version": "0.16.0",
                    "base_url": "http://127.0.0.1:7842",
                    "state_url": "http://127.0.0.1:7842/api/state",
                    "ws_url": "ws://127.0.0.1:7842/ws?token=test",
                    "auth_mode": "ephemeral-bearer"
                },
                "runtime": {
                    "backend": "local-process",
                    "host": "desktop:local",
                    "pid": 7842
                },
                "session": {
                    "session_id": "session_01HVK6"
                },
                "policy": {
                    "model": "openai:gpt-4.1",
                    "thinking_level": "medium",
                    "capability_tier": "victory"
                }
            }
        }"#;

        let info: OmegonStartupInfo = serde_json::from_str(json).unwrap();
        let descriptor = info.instance_descriptor.as_ref().unwrap();
        assert_eq!(descriptor.identity.instance_id, "omg_primary_01HVK6");
        assert_eq!(descriptor.identity.role, "primary-driver");
        assert_eq!(
            descriptor
                .workspace
                .as_ref()
                .unwrap()
                .workspace_id
                .as_deref(),
            Some("repo:8f2f4c1")
        );
        assert_eq!(descriptor.control_plane.as_ref().unwrap().schema_version, 2);
        assert_eq!(descriptor.runtime.as_ref().unwrap().pid, Some(7842));
        assert_eq!(
            descriptor.session.as_ref().unwrap().session_id.as_deref(),
            Some("session_01HVK6")
        );
        assert_eq!(
            descriptor.policy.as_ref().unwrap().model.as_deref(),
            Some("openai:gpt-4.1")
        );
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
    fn deserialize_state_payload_with_instance_descriptors() {
        let json = r#"{
            "design": {"focused": null, "implementing": [], "actionable": [], "all_nodes": [], "counts": {}},
            "openspec": {"total_tasks": 0, "done_tasks": 0},
            "cleave": {"active": false, "total_children": 0, "completed": 0, "failed": 0},
            "session": {"turns": 3, "tool_calls": 9, "compactions": 1},
            "instance_descriptor": {
                "identity": {
                    "instance_id": "omg_primary_01HVSTATE",
                    "role": "primary-driver",
                    "profile": "primary-interactive",
                    "status": "busy"
                },
                "workspace": {
                    "cwd": "/repo/state",
                    "workspace_id": "repo:state",
                    "branch": "feature/instance"
                },
                "control_plane": {
                    "schema_version": 2,
                    "omegon_version": "0.16.0",
                    "base_url": "http://127.0.0.1:7843",
                    "state_url": "http://127.0.0.1:7843/api/state",
                    "ready_url": "http://127.0.0.1:7843/api/readyz",
                    "ws_url": "ws://127.0.0.1:7843/ws?token=test",
                    "auth_mode": "ephemeral-bearer",
                    "token_ref": "secret://auspex/instances/omg_primary_01HVSTATE/token",
                    "last_ready_at": "2026-04-05T10:00:00Z"
                },
                "runtime": {
                    "backend": "local-process",
                    "host": "desktop:local",
                    "placement_id": "pid/8123",
                    "pid": 8123
                },
                "session": {
                    "session_id": "session_01HVSTATE"
                },
                "policy": {
                    "model": "anthropic:claude-sonnet-4-6",
                    "thinking_level": "high",
                    "capability_tier": "gloriana"
                }
            },
            "dispatcher": {
                "session_id": "session_01HVSTATE",
                "dispatcher_instance_id": "legacy-will-be-ignored",
                "expected_role": "legacy-role",
                "expected_profile": "legacy-profile",
                "expected_model": "legacy-model",
                "control_plane_schema": 1,
                "instance_descriptor": {
                    "identity": {
                        "instance_id": "omg_dispatcher_01HVSTATE",
                        "role": "primary-driver",
                        "profile": "dispatcher-profile",
                        "status": "ready"
                    },
                    "control_plane": {
                        "schema_version": 2,
                        "base_url": "http://127.0.0.1:7844",
                        "token_ref": "secret://auspex/instances/omg_dispatcher_01HVSTATE/token",
                        "last_verified_at": "2026-04-05T10:01:00Z"
                    },
                    "session": {
                        "session_id": "session_01HVDISPATCH"
                    },
                    "policy": {
                        "model": "openai:gpt-4.1"
                    }
                }
            }
        }"#;

        let snapshot: OmegonStateSnapshot = serde_json::from_str(json).unwrap();
        let descriptor = snapshot.instance_descriptor.as_ref().unwrap();
        assert_eq!(descriptor.identity.instance_id, "omg_primary_01HVSTATE");
        assert_eq!(
            descriptor.workspace.as_ref().unwrap().branch.as_deref(),
            Some("feature/instance")
        );
        assert_eq!(
            descriptor
                .control_plane
                .as_ref()
                .unwrap()
                .token_ref
                .as_deref(),
            Some("secret://auspex/instances/omg_primary_01HVSTATE/token")
        );
        assert_eq!(
            snapshot
                .dispatcher
                .as_ref()
                .unwrap()
                .instance_descriptor
                .as_ref()
                .unwrap()
                .identity
                .instance_id,
            "omg_dispatcher_01HVSTATE"
        );
        assert_eq!(
            snapshot
                .dispatcher
                .as_ref()
                .unwrap()
                .instance_descriptor
                .as_ref()
                .unwrap()
                .control_plane
                .as_ref()
                .unwrap()
                .schema_version,
            2
        );
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
