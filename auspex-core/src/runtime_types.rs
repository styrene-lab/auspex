#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::secret_grants::SecretGrantPrincipal;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandTarget {
    pub session_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatcher_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalSlashCommand {
    pub name: String,
    pub args: String,
    pub raw_input: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperatorCommand {
    PromptSubmit {
        text: String,
    },
    TurnCancel,
    CanonicalSlash {
        slash: CanonicalSlashCommand,
    },
    DispatcherSwitch {
        request_id: String,
        profile: String,
        model: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetedCommandEnvelope {
    pub target: CommandTarget,
    pub command: OperatorCommand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetedCommand {
    pub target: CommandTarget,
    pub command: OperatorCommand,
}

impl TargetedCommand {
    pub fn prompt_submit(target: CommandTarget, text: impl Into<String>) -> Self {
        Self {
            target,
            command: OperatorCommand::PromptSubmit { text: text.into() },
        }
    }

    pub fn turn_cancel(target: CommandTarget) -> Self {
        Self {
            target,
            command: OperatorCommand::TurnCancel,
        }
    }

    pub fn canonical_slash(target: CommandTarget, slash: CanonicalSlashCommand) -> Self {
        Self {
            target,
            command: OperatorCommand::CanonicalSlash { slash },
        }
    }

    pub fn dispatcher_switch(
        target: CommandTarget,
        request_id: impl Into<String>,
        profile: impl Into<String>,
        model: Option<String>,
    ) -> Self {
        Self {
            target,
            command: OperatorCommand::DispatcherSwitch {
                request_id: request_id.into(),
                profile: profile.into(),
                model,
            },
        }
    }

    pub fn web_command_json(&self) -> String {
        match &self.command {
            OperatorCommand::PromptSubmit { text } => serde_json::json!({
                "type": "user_prompt",
                "text": text,
            })
            .to_string(),
            OperatorCommand::TurnCancel => serde_json::json!({ "type": "cancel" }).to_string(),
            OperatorCommand::CanonicalSlash { slash } => serde_json::json!({
                "type": "slash_command",
                "name": slash.name,
                "args": slash.args,
            })
            .to_string(),
            OperatorCommand::DispatcherSwitch {
                request_id,
                profile,
                model,
            } => serde_json::json!({
                "type": "switch_dispatcher",
                "request_id": request_id,
                "profile": profile,
                "model": model,
            })
            .to_string(),
        }
    }

    pub fn transport_envelope(&self) -> TargetedCommandEnvelope {
        TargetedCommandEnvelope {
            target: self.target.clone(),
            command: self.command.clone(),
        }
    }

    pub fn transport_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.transport_envelope())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceRecord {
    pub schema_version: u32,
    pub identity: WorkerIdentity,
    pub ownership: WorkerOwnership,
    pub desired: DesiredWorkerState,
    pub observed: ObservedWorkerState,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerIdentity {
    pub instance_id: String,
    pub role: WorkerRole,
    pub profile: String,
    pub status: WorkerLifecycleState,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerOwnership {
    pub owner_kind: OwnerKind,
    pub owner_id: String,
    #[serde(default)]
    pub parent_instance_id: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesiredWorkerState {
    pub backend: BackendConfig,
    pub workspace: WorkspaceBinding,
    #[serde(default)]
    pub task: Option<TaskBinding>,
    #[serde(default)]
    pub policy: PolicyOverrides,
    #[serde(default)]
    pub security: WorkerSecurityBinding,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedWorkerState {
    pub placement: ObservedPlacement,
    pub control_plane: ObservedControlPlane,
    pub health: ObservedHealth,
    pub exit: ObservedExit,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceBinding {
    pub cwd: String,
    pub workspace_id: String,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskBinding {
    pub task_id: String,
    pub purpose: String,
    #[serde(default)]
    pub spec_binding: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PolicyOverrides {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub thinking_level: Option<ThinkingLevel>,
    #[serde(default)]
    pub context_class: Option<String>,
    #[serde(default)]
    pub tool_policy: Option<ToolPolicy>,
    #[serde(default)]
    pub memory_mode: Option<MemoryMode>,
    #[serde(default)]
    pub max_runtime_seconds: Option<u64>,
    #[serde(default)]
    pub max_cost_usd: Option<f64>,
}

impl Eq for PolicyOverrides {}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedPlacement {
    pub placement_id: String,
    pub host: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub pod_name: Option<String>,
    #[serde(default)]
    pub container_name: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedControlPlane {
    pub schema_version: u32,
    pub omegon_version: String,
    pub base_url: String,
    pub startup_url: String,
    pub health_url: String,
    pub ready_url: String,
    pub ws_url: String,
    #[serde(default)]
    pub acp_url: Option<String>,
    pub auth_mode: String,
    #[serde(default)]
    pub token_ref: Option<String>,
    #[serde(default)]
    pub last_ready_at: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedHealth {
    pub ready: bool,
    #[serde(default)]
    pub degraded_reason: Option<String>,
    #[serde(default)]
    pub last_heartbeat_at: Option<String>,
    #[serde(default)]
    pub last_seen_at: Option<String>,
    #[serde(default)]
    pub freshness: Option<InstanceFreshness>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceFreshness {
    #[default]
    Fresh,
    Stale,
    Lost,
    Abandoned,
    Reaped,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedExit {
    pub exited: bool,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub exit_reason: Option<String>,
    #[serde(default)]
    pub exited_at: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstantiateRequest {
    pub schema_version: u32,
    pub role: WorkerRole,
    pub profile: String,
    pub backend: BackendKind,
    pub workspace: WorkspaceBinding,
    #[serde(default)]
    pub parent_instance_id: Option<String>,
    #[serde(default)]
    pub task: Option<TaskBinding>,
    #[serde(default)]
    pub overrides: InstantiateOverrides,
    #[serde(default)]
    pub security: WorkerSecurityBinding,
    #[serde(default)]
    pub propagation: Option<WorkerPropagation>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct InstantiateOverrides {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub thinking_level: Option<ThinkingLevel>,
    #[serde(default)]
    pub max_runtime_seconds: Option<u64>,
    #[serde(default)]
    pub max_cost_usd: Option<f64>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub resources: Option<ResourceRequirements>,
    #[serde(default)]
    pub tool_policy: Option<ToolPolicy>,
    #[serde(default)]
    pub memory_mode: Option<MemoryMode>,
}

impl Eq for InstantiateOverrides {}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceRequirements {
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerSecurityBinding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<SecretGrantPrincipal>,
    #[serde(default)]
    pub secret_refs: Vec<String>,
    #[serde(default)]
    pub grant_ids: Vec<String>,
    #[serde(default)]
    pub seed_plan_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerPropagation {
    pub workspace: WorkspaceBinding,
    #[serde(default)]
    pub task_context: Option<PropagatedTaskContext>,
    pub auth: PropagatedAuth,
    pub policy: ResolvedPolicy,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PropagatedTaskContext {
    pub task_id: String,
    pub prompt: String,
    #[serde(default)]
    pub design_refs: Vec<String>,
    #[serde(default)]
    pub spec_refs: Vec<String>,
    #[serde(default)]
    pub memory_refs: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PropagatedAuth {
    #[serde(default)]
    pub provider_refs: Vec<String>,
    pub secret_mode: SecretMode,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ResolvedPolicy {
    pub base_profile: String,
    #[serde(default)]
    pub resolved_model: Option<String>,
    pub thinking_level: ThinkingLevel,
    pub tool_policy: ToolPolicy,
    pub memory_mode: MemoryMode,
    #[serde(default)]
    pub max_runtime_seconds: Option<u64>,
    #[serde(default)]
    pub max_cost_usd: Option<f64>,
}

impl Eq for ResolvedPolicy {}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendConfig {
    pub kind: BackendKind,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub resources: Option<ResourceRequirements>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WorkerProfilesFile {
    pub version: u32,
    pub profiles: std::collections::BTreeMap<String, WorkerProfile>,
}

impl Eq for WorkerProfilesFile {}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct WorkerProfile {
    pub role: WorkerRole,
    #[serde(default)]
    pub preferred_models: Vec<String>,
    #[serde(default)]
    pub fallback_models: Vec<String>,
    pub thinking_level: ThinkingLevel,
    pub context_class: String,
    pub tool_policy: ToolPolicy,
    pub memory_mode: MemoryMode,
    pub max_runtime_seconds: u64,
    pub max_cost_usd: f64,
    #[serde(default)]
    pub parallelism_limit: Option<u32>,
    #[serde(default)]
    pub network_policy: Option<String>,
}

impl Eq for WorkerProfile {}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerRole {
    #[default]
    PrimaryDriver,
    SupervisedChild,
    DetachedService,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OwnerKind {
    #[default]
    AuspexSession,
    DaemonOwned,
    External,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerLifecycleState {
    #[default]
    Requested,
    Allocating,
    Starting,
    Ready,
    Busy,
    Degraded,
    Lost,
    Abandoned,
    Stopping,
    Exited,
    Reaped,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    #[default]
    LocalProcess,
    LocalDetached,
    OciContainer,
    Kubernetes,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ThinkingLevel {
    #[default]
    Minimal,
    Low,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolPolicy {
    #[default]
    Full,
    Restricted,
    Bounded,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryMode {
    #[default]
    Full,
    Minimal,
    ProjectOnly,
    ReferenceBased,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SecretMode {
    #[default]
    Reference,
    Mounted,
    InheritedEnv,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn deserialize_instance_record_from_registry_shape() {
        let json = r#"{
          "schema_version": 1,
          "identity": {
            "instance_id": "omg_01HVK6K4QFQF8B2W2J7Q6M7Y3S",
            "role": "supervised-child",
            "profile": "cheap-subtask",
            "status": "busy",
            "created_at": "2026-04-03T12:00:00Z",
            "updated_at": "2026-04-03T12:03:42Z"
          },
          "ownership": {
            "owner_kind": "auspex-session",
            "owner_id": "session_01HV...",
            "parent_instance_id": "omg_primary_01HV..."
          },
          "desired": {
            "backend": {
              "kind": "kubernetes",
              "image": "ghcr.io/org/omegon:v0.15.7",
              "namespace": "auspex",
              "resources": {
                "cpu": "500m",
                "memory": "1Gi"
              }
            },
            "workspace": {
              "cwd": "/repo/path",
              "workspace_id": "repo:8f2f4c1",
              "branch": "main"
            },
            "task": {
              "task_id": "clv-child-2",
              "purpose": "parallel subtask",
              "spec_binding": "auspex-data-model-v2"
            },
            "policy": {
              "provider": null,
              "model": null,
              "thinking_level": null,
              "context_class": null,
              "tool_policy": null,
              "memory_mode": null,
              "max_runtime_seconds": 900,
              "max_cost_usd": 0.5
            }
          },
          "observed": {
            "placement": {
              "placement_id": "pod/auspex/omegon-child-abc123",
              "host": "cluster:dev-us-east-1",
              "pid": null,
              "namespace": "auspex",
              "pod_name": "omegon-child-abc123",
              "container_name": "omegon"
            },
            "control_plane": {
              "schema_version": 2,
              "omegon_version": "0.15.7",
              "base_url": "http://omegon-child-abc123.auspex.svc:7842",
              "startup_url": "http://omegon-child-abc123.auspex.svc:7842/api/startup",
              "health_url": "http://omegon-child-abc123.auspex.svc:7842/api/healthz",
              "ready_url": "http://omegon-child-abc123.auspex.svc:7842/api/readyz",
              "ws_url": "ws://omegon-child-abc123.auspex.svc:7842/ws?token=...",
              "auth_mode": "ephemeral-bearer",
              "token_ref": "secret://auspex/instances/omg_01HV.../token",
              "last_ready_at": "2026-04-03T12:00:11Z"
            },
            "health": {
              "ready": true,
              "degraded_reason": null,
              "last_heartbeat_at": "2026-04-03T12:03:42Z"
            },
            "exit": {
              "exited": false,
              "exit_code": null,
              "exit_reason": null,
              "exited_at": null
            }
          }
        }"#;

        let record: InstanceRecord = serde_json::from_str(json).unwrap();

        assert_eq!(record.schema_version, 1);
        assert_eq!(record.identity.role, WorkerRole::SupervisedChild);
        assert_eq!(record.identity.status, WorkerLifecycleState::Busy);
        assert_eq!(record.ownership.owner_kind, OwnerKind::AuspexSession);
        assert_eq!(record.desired.backend.kind, BackendKind::Kubernetes);
        assert_eq!(record.desired.policy.max_runtime_seconds, Some(900));
        assert_eq!(
            record.observed.control_plane.token_ref.as_deref(),
            Some("secret://auspex/instances/omg_01HV.../token")
        );
        assert!(record.observed.health.ready);
    }

    #[test]
    fn deserialize_instantiate_request_with_propagation() {
        let json = r#"{
          "schema_version": 1,
          "role": "supervised-child",
          "profile": "cheap-subtask",
          "backend": "kubernetes",
          "workspace": {
            "cwd": "/repo/path",
            "workspace_id": "repo:8f2f4c1",
            "branch": "main"
          },
          "parent_instance_id": "omg_primary_01HV...",
          "task": {
            "task_id": "clv-child-2",
            "purpose": "parallel subtask",
            "spec_binding": "auspex-data-model-v2"
          },
          "overrides": {
            "model": "anthropic:claude-haiku",
            "thinking_level": "low",
            "max_runtime_seconds": 900,
            "image": "ghcr.io/org/omegon:v0.15.7",
            "namespace": "auspex",
            "resources": {
              "cpu": "500m",
              "memory": "1Gi"
            }
          },
          "security": {
            "principal": {
              "kind": "spiffe",
              "id": "spiffe://styrene.dev/agents/clv-child-2"
            },
            "secret_refs": ["provider.anthropic"],
            "grant_ids": ["grant_clv_child_2"]
          },
          "propagation": {
            "workspace": {
              "cwd": "/repo/path",
              "workspace_id": "repo:8f2f4c1",
              "branch": "main"
            },
            "task_context": {
              "task_id": "clv-child-2",
              "prompt": "Implement the schema projection for tool cards",
              "design_refs": ["auspex-data-model-v2"],
              "spec_refs": ["auspex-data-model-v2"],
              "memory_refs": ["fact_123", "fact_456"]
            },
            "auth": {
              "provider_refs": ["anthropic", "openai"],
              "secret_mode": "reference"
            },
            "policy": {
              "base_profile": "cheap-subtask",
              "resolved_model": "anthropic:claude-haiku",
              "thinking_level": "low",
              "tool_policy": "restricted",
              "memory_mode": "project-only"
            }
          }
        }"#;

        let request: InstantiateRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.role, WorkerRole::SupervisedChild);
        assert_eq!(request.backend, BackendKind::Kubernetes);
        assert_eq!(request.overrides.thinking_level, Some(ThinkingLevel::Low));
        assert_eq!(
            request.propagation.as_ref().unwrap().policy.memory_mode,
            MemoryMode::ProjectOnly
        );
        assert_eq!(
            request.propagation.as_ref().unwrap().auth.provider_refs,
            vec!["anthropic", "openai"]
        );
        assert_eq!(
            request.security.principal.as_ref().unwrap().id,
            "spiffe://styrene.dev/agents/clv-child-2"
        );
        assert_eq!(request.security.secret_refs, vec!["provider.anthropic"]);
    }

    #[test]
    fn targeted_command_serializes_turn_cancel_envelope() {
        let command = TargetedCommand::turn_cancel(CommandTarget {
            session_key: "remote:session_01HVDEMO".into(),
            dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
        });

        assert_eq!(
            command.transport_json().unwrap(),
            r#"{"target":{"session_key":"remote:session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO"},"command":{"kind":"turn_cancel"}}"#
        );
    }

    #[test]
    fn targeted_command_serializes_dispatcher_switch_envelope() {
        let command = TargetedCommand::dispatcher_switch(
            CommandTarget {
                session_key: "remote:session_01HVDEMO".into(),
                dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
            },
            "dispatcher-switch-1",
            "supervisor-heavy",
            Some("openai:gpt-4.1".into()),
        );

        assert_eq!(
            command.transport_json().unwrap(),
            r#"{"target":{"session_key":"remote:session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO"},"command":{"kind":"dispatcher_switch","request_id":"dispatcher-switch-1","profile":"supervisor-heavy","model":"openai:gpt-4.1"}}"#
        );
    }

    #[test]
    fn targeted_command_serializes_canonical_slash_envelope() {
        let command = TargetedCommand::canonical_slash(
            CommandTarget {
                session_key: "remote:session_01HVDEMO".into(),
                dispatcher_instance_id: Some("omg_primary_01HVDEMO".into()),
            },
            CanonicalSlashCommand {
                name: "login".into(),
                args: "anthropic".into(),
                raw_input: "/login anthropic".into(),
            },
        );

        assert_eq!(
            command.web_command_json(),
            r#"{"args":"anthropic","name":"login","type":"slash_command"}"#
        );
        assert_eq!(
            command.transport_json().unwrap(),
            r#"{"target":{"session_key":"remote:session_01HVDEMO","dispatcher_instance_id":"omg_primary_01HVDEMO"},"command":{"kind":"canonical_slash","slash":{"name":"login","args":"anthropic","raw_input":"/login anthropic"}}}"#
        );
    }

    #[test]
    fn deserialize_worker_profiles_toml() {
        let toml_text = r#"
version = 1

[profiles.primary-interactive]
role = "primary-driver"
preferred_models = ["anthropic:claude-sonnet-4-6", "openai:gpt-4.1"]
fallback_models = ["anthropic:claude-haiku", "openai:gpt-4.1-mini"]
thinking_level = "medium"
context_class = "clan"
tool_policy = "full"
memory_mode = "full"
max_runtime_seconds = 0
max_cost_usd = 0.0

[profiles.cheap-subtask]
role = "supervised-child"
preferred_models = ["anthropic:claude-haiku", "gpt-spark", "openai:gpt-4.1-mini"]
fallback_models = ["local:qwen2.5-coder"]
thinking_level = "low"
context_class = "squad"
tool_policy = "restricted"
memory_mode = "minimal"
max_runtime_seconds = 900
max_cost_usd = 0.5
parallelism_limit = 4
network_policy = "restricted"
"#;

        let profiles: WorkerProfilesFile = toml::from_str(toml_text).unwrap();

        assert_eq!(profiles.version, 1);
        assert_eq!(profiles.profiles.len(), 2);

        let primary = profiles.profiles.get("primary-interactive").unwrap();
        assert_eq!(primary.role, WorkerRole::PrimaryDriver);
        assert_eq!(primary.thinking_level, ThinkingLevel::Medium);
        assert_eq!(primary.tool_policy, ToolPolicy::Full);

        let child = profiles.profiles.get("cheap-subtask").unwrap();
        assert_eq!(child.role, WorkerRole::SupervisedChild);
        assert_eq!(child.memory_mode, MemoryMode::Minimal);
        assert_eq!(child.parallelism_limit, Some(4));
        assert_eq!(child.network_policy.as_deref(), Some("restricted"));
    }
}
