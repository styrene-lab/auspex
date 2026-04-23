//! OmegonAgent Custom Resource Definition.

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An ExternalAgent points the operator at an omegon instance running outside
/// the cluster (docker-compose, bare-metal, another cloud). The operator does
/// not manage its lifecycle — it only monitors health and proxies connections.
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "styrene.sh",
    version = "v1alpha1",
    kind = "ExternalAgent",
    plural = "externalagents",
    shortname = "xag",
    status = "ExternalAgentStatus",
    namespaced
)]
pub struct ExternalAgentSpec {
    /// Human-readable label for this agent.
    pub display_name: String,

    /// Base URL of the omegon control plane (e.g. "https://agent.example.com:7842").
    pub endpoint: String,

    /// Secret name containing the WebSocket auth token (key: "ws-token").
    /// Tokens must not be placed inline in the CRD spec — they would be
    /// stored in plaintext in etcd and visible to anyone with `get` RBAC.
    #[serde(default)]
    pub token_secret: Option<String>,

    /// Health probe interval in seconds (default: 30).
    #[serde(default = "default_probe_interval")]
    pub probe_interval_seconds: u32,

    /// Agent metadata (informational, not enforced).
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

fn default_probe_interval() -> u32 {
    30
}

/// Status for an ExternalAgent — reflects what the operator can observe.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ExternalAgentStatus {
    /// Reachability: Online, Unreachable, Degraded, Unknown.
    #[serde(default)]
    pub reachability: String,

    /// Omegon version reported by the agent's /api/startup endpoint.
    #[serde(default)]
    pub omegon_version: Option<String>,

    /// Agent ID reported by the agent.
    #[serde(default)]
    pub agent_id: Option<String>,

    /// Model currently in use.
    #[serde(default)]
    pub model: Option<String>,

    /// Last successful health probe (Unix epoch seconds).
    #[serde(default)]
    pub last_seen: Option<String>,

    /// Last probe error message.
    #[serde(default)]
    pub last_error: Option<String>,

    /// WebSocket URL derived from the endpoint.
    #[serde(default)]
    pub ws_url: Option<String>,

    /// SBOM status (if the agent reports it).
    #[serde(default)]
    pub sbom: Option<SbomStatus>,
}

/// An OmegonAgent declares the desired state of a running omegon agent.
/// The operator reconciles this into Deployments (daemon mode), CronJobs
/// (cronjob mode), or Jobs (job mode), plus ConfigMaps and Secrets.
#[derive(CustomResource, Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[kube(
    group = "styrene.sh",
    version = "v1alpha1",
    kind = "OmegonAgent",
    plural = "omegonagents",
    shortname = "oag",
    status = "OmegonAgentStatus",
    namespaced,
    printcolumn = r#"{"name": "Mode", "type": "string", "jsonPath": ".spec.mode"}"#,
    printcolumn = r#"{"name": "Agent", "type": "string", "jsonPath": ".spec.agent"}"#,
    printcolumn = r#"{"name": "Phase", "type": "string", "jsonPath": ".status.phase"}"#
)]
pub struct OmegonAgentSpec {
    /// Agent bundle ID from the catalog (e.g. "styrene.overnight-reviewer").
    pub agent: String,

    /// LLM provider and model (e.g. "openai-codex:gpt-5.4", "anthropic:claude-sonnet-4-6",
    /// "google:gemini-2.5-flash").
    pub model: String,

    /// Agent posture: explorator, fabricator (default), architect, devastator.
    /// Controls behavioral mode — tool surface, delegation strategy, reasoning depth.
    #[serde(default = "default_posture")]
    pub posture: String,

    /// Worker role in swarm hierarchy: primary-driver (supervisor), supervised-child
    /// (worker), or detached-service (sentry/monitor). Maps to aether's tier system
    /// for RBAC and delegation authority.
    #[serde(default = "default_role")]
    pub role: String,

    /// "daemon" for long-lived bots, "cronjob" for scheduled, "job" for bounded oneshot.
    #[serde(default = "default_mode")]
    pub mode: AgentMode,

    /// Cron schedule (required when mode=cronjob).
    #[serde(default)]
    pub schedule: Option<String>,

    /// Container image. When `profile` is set and `image` is empty, the image
    /// name is derived from the profile's `[meta] name` field.
    #[serde(default = "default_image")]
    pub image: String,

    /// Nex profile reference for image building (e.g. "styrene-lab/omegon-rust-dev").
    /// When set, CI builds the image from this profile via `nex build-image`.
    #[serde(default)]
    pub profile: Option<String>,

    /// Vox connector configuration.
    #[serde(default)]
    pub vox: VoxSpec,

    /// Secret references.
    #[serde(default)]
    pub secrets: SecretsSpec,

    /// Resource requests/limits.
    #[serde(default)]
    pub resources: Option<ResourcesSpec>,

    /// Mesh identity configuration. Controls how the agent's StyreneID
    /// is derived and how it joins the Styrene mesh.
    #[serde(default)]
    pub identity: Option<IdentitySpec>,

    /// Resource bounds for bounded execution (job/cronjob modes).
    #[serde(default)]
    pub bounds: Option<BoundsSpec>,

    /// Prompt source for job/cronjob modes.
    #[serde(default)]
    pub prompt: Option<PromptSpec>,

    /// SBOM tracking and verification configuration.
    #[serde(default)]
    pub sbom: Option<SbomSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    Daemon,
    Cronjob,
    Job,
}

fn default_mode() -> AgentMode {
    AgentMode::Cronjob
}

fn default_posture() -> String {
    "fabricator".to_string()
}

fn default_role() -> String {
    "supervised-child".to_string()
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

    /// Mount auth.json from a Secret for OAuth tokens.
    #[serde(default)]
    pub auth_json_secret: Option<String>,

    /// HashiCorp Vault integration for secret injection.
    #[serde(default)]
    pub vault: Option<VaultSpec>,
}

/// HashiCorp Vault integration for agent secret injection.
///
/// When configured, the operator annotates the pod for the Vault Agent
/// injector (or CSI driver) to inject secrets directly into the container.
/// Secrets never pass through the operator's memory or the k8s Secret API.
///
/// This covers:
/// - LLM provider API keys (ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)
/// - Omegon auth.json (OAuth tokens for all providers)
/// - Bot tokens (Discord, Slack)
/// - Extension secrets
/// - Operator root identity (for Tier 2+ security postures)
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct VaultSpec {
    /// Vault server address (e.g. "https://vault.internal:8200").
    /// When empty, uses VAULT_ADDR from the operator's environment.
    #[serde(default)]
    pub address: Option<String>,

    /// Vault auth method: "kubernetes" (default) or "approle".
    #[serde(default = "default_vault_auth_method")]
    pub auth_method: String,

    /// Vault role for k8s auth (used with auth_method=kubernetes).
    #[serde(default)]
    pub role: Option<String>,

    /// Vault secret paths to inject. Each entry maps a Vault path to a
    /// container file path or environment variable.
    #[serde(default)]
    pub secrets: Vec<VaultSecretMapping>,

    /// Whether to use the Vault Agent sidecar injector (default: true).
    /// When false, uses the Vault CSI driver instead.
    #[serde(default = "default_true")]
    pub agent_inject: bool,
}

fn default_vault_auth_method() -> String {
    "kubernetes".to_string()
}

fn default_true() -> bool {
    true
}

/// Maps a Vault secret path to a container destination.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VaultSecretMapping {
    /// Vault secret path (e.g. "secret/data/agents/my-agent/provider-keys").
    pub path: String,

    /// Destination inside the container.
    /// File path (e.g. "/config/omegon/auth.json") or env prefix.
    pub destination: String,

    /// Template for rendering the secret. When empty, injects raw JSON.
    /// Uses Vault Agent template syntax (Go templates).
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ResourcesSpec {
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
}

/// Mesh identity configuration for a managed agent.
///
/// The operator derives a per-agent StyreneID from its own root secret
/// using the HKDF hierarchy: `HKDF(operator_root, "styrene-agent-master-v1", agent_name)`.
/// This gives the agent a deterministic, recoverable identity that the
/// operator can pre-authorize in the mesh policy before the pod starts.
///
/// ## Security Tiers
///
/// The `security_tier` field controls where the operator's root secret lives
/// and how agent keys are derived. Pick the tier that matches your threat model:
///
/// ### Tier 1 — File-based (development, solo operators)
///   Root secret in a k8s Secret, operator reads it directly.
///   Good enough for dev clusters and single-operator deployments.
///   ```yaml
///   identity:
///     provision: true
///     securityTier: file
///   ```
///
/// ### Tier 2 — Vault-backed (production, team deployments)
///   Root secret stored in HashiCorp Vault. The operator reads it via
///   k8s auth, derives agent keys, and writes them back to Vault
///   (not k8s Secrets). Agent pods read their keys via Vault Agent
///   sidecar — the secret never touches the k8s Secret API.
///   ```yaml
///   identity:
///     provision: true
///     securityTier: vault
///     vaultPath: "secret/data/styrene/operator-root"
///   ```
///
/// ### Tier 3 — HSM-backed (high-security, compliance)
///   Root secret lives in a hardware security module (YubiKey, CloudHSM,
///   PKCS#11). The operator never sees the root — it sends derivation
///   requests to the HSM, which returns only the derived agent key.
///   Requires `styrene-identity` with `yubikey` feature and a mounted
///   FIDO2/PIV device or CloudHSM socket.
///   ```yaml
///   identity:
///     provision: true
///     securityTier: hsm
///     hsmSlot: "pkcs11:token=styrene-operator"
///   ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct IdentitySpec {
    /// Whether to provision a StyreneID for this agent's styrened sidecar.
    #[serde(default)]
    pub provision: bool,

    /// Security tier: "file" (default), "vault", or "hsm".
    /// See struct-level docs for details on each tier.
    #[serde(default = "default_security_tier")]
    pub security_tier: String,

    /// RBAC mesh role for this agent: observer, monitor, operator, admin.
    #[serde(default = "default_mesh_role")]
    pub mesh_role: String,

    /// Override the derivation label (defaults to "{namespace}/{name}").
    #[serde(default)]
    pub derivation_label: Option<String>,

    // --- Tier 1 (file) fields ---

    /// Name of the k8s Secret containing the operator's root secret.
    /// Used when security_tier=file.
    #[serde(default = "default_operator_secret")]
    pub operator_secret: String,

    /// Key within the operator Secret that holds the root secret bytes.
    #[serde(default = "default_operator_secret_key")]
    pub operator_secret_key: String,

    // --- Tier 2 (vault) fields ---

    /// Vault path for the operator's root secret.
    /// Used when security_tier=vault.
    #[serde(default)]
    pub vault_path: Option<String>,

    /// Vault path prefix for derived agent secrets.
    /// Agent keys are written to "{vault_agent_prefix}/{agent_name}".
    #[serde(default)]
    pub vault_agent_prefix: Option<String>,

    // --- Tier 3 (hsm) fields ---

    /// PKCS#11 URI or device path for the HSM.
    /// Used when security_tier=hsm.
    #[serde(default)]
    pub hsm_slot: Option<String>,

    // --- Common fields ---

    /// Whether to rotate the agent's identity on the next reconciliation.
    #[serde(default)]
    pub rotate: bool,

    /// Additional RNS destinations this agent should be able to reach.
    #[serde(default)]
    pub mesh_peers: Vec<String>,

    /// Enable mTLS on the fleet API using StyreneIdentity-derived certificates.
    /// When true, the operator generates a self-signed CA from its identity and
    /// issues per-client certificates for Auspex desktop/web connections.
    #[serde(default)]
    pub mtls: bool,
}

fn default_security_tier() -> String {
    "file".to_string()
}

fn default_mesh_role() -> String {
    "operator".to_string()
}

fn default_operator_secret() -> String {
    "styrene-operator-identity".to_string()
}

fn default_operator_secret_key() -> String {
    "root-secret".to_string()
}

/// Resource bounds for bounded agent execution (job/cronjob modes).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct BoundsSpec {
    /// Maximum number of agent turns before exit.
    #[serde(default)]
    pub max_turns: Option<u32>,

    /// Wall-clock timeout in seconds. Agent exits cleanly at this limit.
    #[serde(default)]
    pub timeout: Option<u32>,

    /// Maximum total input+output tokens. Prevents runaway cost.
    #[serde(default)]
    pub token_budget: Option<u64>,

    /// Context class: squad, maniple, clan, legion.
    #[serde(default)]
    pub context_class: Option<String>,

    /// k8s activeDeadlineSeconds for the Job (hard ceiling).
    #[serde(default)]
    pub active_deadline_seconds: Option<i64>,
}

/// Prompt source for job/cronjob modes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PromptSpec {
    /// Inline prompt text.
    #[serde(default)]
    pub inline: Option<String>,

    /// ConfigMap name containing the prompt (key: "prompt.txt").
    #[serde(default)]
    pub config_map: Option<String>,

    /// Secret name containing the prompt (key: "prompt.txt").
    /// Use instead of config_map when the prompt contains sensitive instructions.
    #[serde(default)]
    pub secret: Option<String>,

    /// Mount path for prompt file inside the container.
    #[serde(default = "default_prompt_path")]
    pub mount_path: String,

    /// Path for structured output inside the container.
    #[serde(default = "default_output_path")]
    pub output_path: String,
}

fn default_prompt_path() -> String {
    "/input/prompt.txt".to_string()
}

fn default_output_path() -> String {
    "/output/result.json".to_string()
}

/// SBOM tracking and verification configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SbomSpec {
    /// Whether SBOM generation is enabled for this agent's image.
    #[serde(default = "default_sbom_enabled")]
    pub enabled: bool,

    /// SBOM output format: "cyclonedx" (default) or "spdx".
    #[serde(default = "default_sbom_format")]
    pub format: String,

    /// OCI registry reference where the SBOM artifact is stored.
    /// When empty, derived from the image reference: `<image>-sbom.cdx.json`.
    #[serde(default)]
    pub artifact_ref: Option<String>,

    /// Whether to run grype vulnerability scanning against the SBOM.
    #[serde(default)]
    pub vulnerability_scan: bool,
}

fn default_sbom_enabled() -> bool {
    true
}

fn default_sbom_format() -> String {
    "cyclonedx".to_string()
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

    /// Last successful run (for cronjob/job mode).
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

    /// SBOM status for the running image.
    #[serde(default)]
    pub sbom: Option<SbomStatus>,

    /// Mesh identity status.
    #[serde(default)]
    pub identity: Option<IdentityStatus>,
}

/// Mesh identity status tracked per managed agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct IdentityStatus {
    /// Whether a StyreneID has been provisioned for this agent.
    #[serde(default)]
    pub provisioned: bool,

    /// The k8s Secret name holding the derived agent root secret.
    #[serde(default)]
    pub secret_name: Option<String>,

    /// The agent's RNS destination hash (derived from the identity).
    #[serde(default)]
    pub rns_destination_hash: Option<String>,

    /// The agent's WireGuard public key (derived from the identity).
    #[serde(default)]
    pub wireguard_pubkey: Option<String>,

    /// Assigned mesh role.
    #[serde(default)]
    pub mesh_role: Option<String>,

    /// Whether the agent has been admitted to the mesh
    /// (operator has added it to the mesh policy).
    #[serde(default)]
    pub mesh_admitted: bool,

    /// Last identity provisioning timestamp (ISO 8601).
    #[serde(default)]
    pub provisioned_at: Option<String>,
}

/// SBOM status tracked per agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct SbomStatus {
    /// Whether an SBOM exists for the current image digest.
    #[serde(default)]
    pub available: bool,

    /// OCI artifact reference for the SBOM (e.g. ghcr.io/org/image:sha256-abc.sbom).
    #[serde(default)]
    pub artifact_ref: Option<String>,

    /// SBOM format: "cyclonedx" or "spdx".
    #[serde(default)]
    pub format: Option<String>,

    /// Image digest the SBOM was generated for.
    #[serde(default)]
    pub image_digest: Option<String>,

    /// When the SBOM was last generated (ISO 8601).
    #[serde(default)]
    pub generated_at: Option<String>,

    /// Number of components in the SBOM.
    #[serde(default)]
    pub component_count: Option<u32>,

    /// Number of known vulnerabilities (from grype scan, if enabled).
    #[serde(default)]
    pub vulnerability_count: Option<u32>,

    /// Cosign signature verification status.
    #[serde(default)]
    pub signature_verified: Option<bool>,
}
