//! OmegonAgent Custom Resource Definition.

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An OmegonAgent declares the desired state of a running omegon agent.
/// The operator reconciles this into Deployments (daemon mode) or CronJobs
/// (oneshot mode), plus ConfigMaps and Secrets.
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "styrene.sh",
    version = "v1alpha1",
    kind = "OmegonAgent",
    plural = "omegonagents",
    shortname = "oag",
    status = "OmegonAgentStatus",
    namespaced
)]
pub struct OmegonAgentSpec {
    /// Agent bundle ID from the catalog (e.g. "styrene.overnight-reviewer").
    pub agent: String,

    /// LLM provider and model (e.g. "openai-codex:gpt-5.4", "anthropic:claude-sonnet-4-6").
    pub model: String,

    /// "daemon" for long-lived bots, "cronjob" for oneshot agents.
    #[serde(default = "default_mode")]
    pub mode: AgentMode,

    /// Cron schedule (required when mode=cronjob).
    #[serde(default)]
    pub schedule: Option<String>,

    /// Container image.
    #[serde(default = "default_image")]
    pub image: String,

    /// Vox connector configuration.
    #[serde(default)]
    pub vox: VoxSpec,

    /// Secret references.
    #[serde(default)]
    pub secrets: SecretsSpec,

    /// Resource requests/limits.
    #[serde(default)]
    pub resources: Option<ResourcesSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Daemon,
    Cronjob,
}

fn default_mode() -> AgentMode {
    AgentMode::Cronjob
}

fn default_image() -> String {
    "ghcr.io/styrene-lab/omegon-agents:latest".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct VoxSpec {
    #[serde(default)]
    pub connectors: Vec<String>,

    #[serde(default)]
    pub discord: Option<DiscordSpec>,

    #[serde(default)]
    pub slack: Option<SlackSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DiscordSpec {
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub require_mention: bool,
    #[serde(default)]
    pub operators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SlackSpec {
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub operators: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SecretsSpec {
    /// Name of a k8s Secret containing API keys and bot tokens.
    #[serde(default)]
    pub secret_name: Option<String>,

    /// Vault path for secret injection (e.g. "secret/data/vox/discord").
    #[serde(default)]
    pub vault_path: Option<String>,

    /// Vault role for k8s auth.
    #[serde(default)]
    pub vault_role: Option<String>,

    /// Mount auth.json from a Secret for OAuth tokens.
    #[serde(default)]
    pub auth_json_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourcesSpec {
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
}

/// Status subresource for OmegonAgent.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct OmegonAgentStatus {
    /// Current phase: Pending, Running, Succeeded, Failed, Unknown.
    #[serde(default)]
    pub phase: String,

    /// Last reconciliation timestamp.
    #[serde(default)]
    pub last_reconciled: Option<String>,

    /// Last successful run (for cronjob mode).
    #[serde(default)]
    pub last_run: Option<String>,

    /// Agent health as reported by /api/healthz.
    #[serde(default)]
    pub health: Option<String>,

    /// Human-readable message.
    #[serde(default)]
    pub message: Option<String>,

    /// Observed generation for status staleness detection.
    #[serde(default)]
    pub observed_generation: Option<i64>,
}
