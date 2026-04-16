//! Configuration loader for Auspex worker and deploy profiles.
//!
//! Follows the same pkl-first/toml-fallback pattern as omegon's
//! `agent_manifest.rs` — evaluates `.pkl` files via `rpkl::from_config`,
//! deserializes directly into serde structs.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::runtime_types::{WorkerProfile, WorkerProfilesFile, WorkerRole};

// ── Deploy profile types ────────────────────────────────────────────────

/// Top-level deploy profiles configuration file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct DeployProfilesFile {
    pub version: u32,
    pub profiles: std::collections::BTreeMap<String, DeployProfile>,
}

impl Eq for DeployProfilesFile {}

/// A single deploy profile defining backend, image, and resource policy.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct DeployProfile {
    pub backend: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub resources: Option<DeployResources>,
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
    #[serde(default)]
    pub omegon_flags: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub max_instances: Option<u32>,
    #[serde(default)]
    pub restart_on_exit: bool,
    /// System tools that must be available in PATH for this profile to
    /// be usable (e.g., "kubectl", "helm", "docker", "podman").
    #[serde(default)]
    pub requires: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct DeployResources {
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct HealthCheckConfig {
    #[serde(default = "default_interval")]
    pub interval_secs: u32,
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_initial_delay")]
    pub initial_delay_secs: u32,
}

fn default_interval() -> u32 {
    30
}
fn default_failure_threshold() -> u32 {
    3
}
fn default_initial_delay() -> u32 {
    10
}

// ── Remote instance registration types ──────────────────────────────────

/// Top-level remote instances configuration file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct RemoteInstancesFile {
    pub version: u32,
    pub instances: std::collections::BTreeMap<String, RemoteInstanceEntry>,
}

/// A single remote instance registration entry.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct RemoteInstanceEntry {
    pub label: String,
    pub base_url: String,
    #[serde(default = "default_detached_role")]
    pub role: WorkerRole,
    #[serde(default = "default_remote_profile")]
    pub profile: String,
    #[serde(default)]
    pub token_ref: Option<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_auth_mode")]
    pub auth_mode: String,
    #[serde(default = "default_auto_attach")]
    pub auto_attach: bool,
}

fn default_detached_role() -> WorkerRole {
    WorkerRole::DetachedService
}

fn default_remote_profile() -> String {
    "remote-agent".into()
}

fn default_auth_mode() -> String {
    "ephemeral-bearer".into()
}

fn default_auto_attach() -> bool {
    true
}

impl RemoteInstanceEntry {
    /// Derive the standard control plane URLs from the base_url.
    pub fn startup_url(&self) -> String {
        format!("{}/api/startup", self.base_url.trim_end_matches('/'))
    }

    pub fn health_url(&self) -> String {
        format!("{}/api/healthz", self.base_url.trim_end_matches('/'))
    }

    pub fn ready_url(&self) -> String {
        format!("{}/api/readyz", self.base_url.trim_end_matches('/'))
    }

    pub fn ws_url(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        let ws_base = base
            .replace("https://", "wss://")
            .replace("http://", "ws://");
        match &self.token_ref {
            Some(token) if !token.is_empty() => format!("{ws_base}/ws?token={token}"),
            _ => format!("{ws_base}/ws"),
        }
    }
}

// ── Resolved config bundle ──────────────────────────────────────────────

/// All loaded configuration, resolved from disk.
#[derive(Clone, Debug, Default)]
pub struct ResolvedConfig {
    pub worker_profiles: WorkerProfilesFile,
    pub deploy_profiles: DeployProfilesFile,
    pub remote_instances: RemoteInstancesFile,
    pub worker_source: Option<PathBuf>,
    pub deploy_source: Option<PathBuf>,
    pub remote_source: Option<PathBuf>,
}

impl ResolvedConfig {
    /// Look up a worker profile by name.
    pub fn worker_profile(&self, name: &str) -> Option<&WorkerProfile> {
        self.worker_profiles.profiles.get(name)
    }

    /// Look up a deploy profile by name.
    pub fn deploy_profile(&self, name: &str) -> Option<&DeployProfile> {
        self.deploy_profiles.profiles.get(name)
    }

    /// Look up a remote instance entry by name.
    pub fn remote_instance(&self, name: &str) -> Option<&RemoteInstanceEntry> {
        self.remote_instances.instances.get(name)
    }

    /// Return remote instance entries that have auto_attach enabled.
    pub fn auto_attach_instances(&self) -> Vec<(&str, &RemoteInstanceEntry)> {
        self.remote_instances
            .instances
            .iter()
            .filter(|(_, entry)| entry.auto_attach)
            .map(|(name, entry)| (name.as_str(), entry))
            .collect()
    }

    /// Collect all unique system tool requirements across deploy profiles.
    pub fn required_tools(&self) -> Vec<String> {
        let mut tools: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for profile in self.deploy_profiles.profiles.values() {
            if let Some(requires) = &profile.requires {
                tools.extend(requires.iter().cloned());
            }
        }
        tools.into_iter().collect()
    }

    /// Check which required system tools are missing from PATH.
    /// Returns `(profile_name, tool_name)` pairs for each missing tool.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn missing_tools(&self) -> Vec<(String, String)> {
        let mut missing = Vec::new();
        for (name, profile) in &self.deploy_profiles.profiles {
            if let Some(requires) = &profile.requires {
                for tool in requires {
                    if !tool_in_path(tool) {
                        missing.push((name.clone(), tool.clone()));
                    }
                }
            }
        }
        missing
    }

    /// Return deploy profile names whose prerequisites are all satisfied.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn available_deploy_profiles(&self) -> Vec<String> {
        self.deploy_profiles
            .profiles
            .iter()
            .filter(|(_, profile)| {
                profile
                    .requires
                    .as_ref()
                    .is_none_or(|reqs| reqs.iter().all(|t| tool_in_path(t)))
            })
            .map(|(name, _)| name.clone())
            .collect()
    }
}

/// Check whether a tool binary exists in PATH.
#[cfg(not(target_arch = "wasm32"))]
fn tool_in_path(tool: &str) -> bool {
    std::process::Command::new("which")
        .arg(tool)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

// ── Config directory resolution ─────────────────────────────────────────

/// Returns the auspex config directory, defaulting to `~/.config/auspex`.
#[cfg(not(target_arch = "wasm32"))]
fn config_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("AUSPEX_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config/auspex"))
}

// ── Generic pkl/toml loader ─────────────────────────────────────────────

/// Load a config file by stem name from a directory.
/// Tries `{stem}.pkl` first, then `{stem}.toml`.
#[cfg(not(target_arch = "wasm32"))]
fn load_config_file<T: for<'de> Deserialize<'de>>(
    dir: &Path,
    stem: &str,
) -> Result<(T, PathBuf), String> {
    let pkl_path = dir.join(format!("{stem}.pkl"));
    let toml_path = dir.join(format!("{stem}.toml"));

    if pkl_path.exists() {
        let value: T = rpkl::from_config(&pkl_path)
            .map_err(|e| format!("{}: {e}", pkl_path.display()))?;
        return Ok((value, pkl_path));
    }

    if toml_path.exists() {
        let content = std::fs::read_to_string(&toml_path)
            .map_err(|e| format!("{}: {e}", toml_path.display()))?;
        let value: T =
            toml::from_str(&content).map_err(|e| format!("{}: {e}", toml_path.display()))?;
        return Ok((value, toml_path));
    }

    Err(format!(
        "no {stem}.pkl or {stem}.toml found in {}",
        dir.display()
    ))
}

// ── Public API ──────────────────────────────────────────────────────────

/// Load all configuration from the auspex config directory.
///
/// Missing files are not errors — the returned config will use defaults
/// for any file that does not exist.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_config() -> ResolvedConfig {
    let Some(dir) = config_dir() else {
        return ResolvedConfig::default();
    };

    let (worker_profiles, worker_source) =
        match load_config_file::<WorkerProfilesFile>(&dir, "worker-profiles") {
            Ok((profiles, path)) => (profiles, Some(path)),
            Err(_) => (WorkerProfilesFile::default(), None),
        };

    let (deploy_profiles, deploy_source) =
        match load_config_file::<DeployProfilesFile>(&dir, "deploy-profiles") {
            Ok((profiles, path)) => (profiles, Some(path)),
            Err(_) => (DeployProfilesFile::default(), None),
        };

    let (remote_instances, remote_source) =
        match load_config_file::<RemoteInstancesFile>(&dir, "remote-instances") {
            Ok((instances, path)) => (instances, Some(path)),
            Err(_) => (RemoteInstancesFile::default(), None),
        };

    ResolvedConfig {
        worker_profiles,
        deploy_profiles,
        remote_instances,
        worker_source,
        deploy_source,
        remote_source,
    }
}

// ── Remote instance → InstanceRecord conversion ────────────────────────

impl RemoteInstanceEntry {
    /// Convert a named remote instance entry into an `InstanceRecord`.
    /// The record starts with observed health = unknown (not probed yet).
    pub fn to_instance_record(&self, name: &str) -> crate::runtime_types::InstanceRecord {
        let base = self.base_url.trim_end_matches('/');
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();

        crate::runtime_types::InstanceRecord {
            schema_version: 1,
            identity: crate::runtime_types::WorkerIdentity {
                instance_id: format!("remote:{name}"),
                role: self.role,
                profile: self.profile.clone(),
                status: crate::runtime_types::WorkerLifecycleState::Requested,
                created_at: now.clone(),
                updated_at: now.clone(),
            },
            ownership: crate::runtime_types::WorkerOwnership {
                owner_kind: crate::runtime_types::OwnerKind::External,
                owner_id: format!("config:remote-instances/{name}"),
                parent_instance_id: None,
            },
            desired: crate::runtime_types::DesiredWorkerState {
                backend: crate::runtime_types::BackendConfig {
                    kind: crate::runtime_types::BackendKind::LocalDetached,
                    ..Default::default()
                },
                workspace: crate::runtime_types::WorkspaceBinding {
                    cwd: "/remote".into(),
                    workspace_id: format!("remote:{name}"),
                    ..Default::default()
                },
                policy: crate::runtime_types::PolicyOverrides::default(),
                task: None,
            },
            observed: crate::runtime_types::ObservedWorkerState {
                placement: crate::runtime_types::ObservedPlacement {
                    placement_id: format!("remote:{name}"),
                    host: base.to_string(),
                    ..Default::default()
                },
                control_plane: crate::runtime_types::ObservedControlPlane {
                    schema_version: 2,
                    omegon_version: String::new(),
                    base_url: base.to_string(),
                    startup_url: self.startup_url(),
                    health_url: self.health_url(),
                    ready_url: self.ready_url(),
                    ws_url: self.ws_url(),
                    auth_mode: self.auth_mode.clone(),
                    token_ref: self.token_ref.clone(),
                    last_ready_at: None,
                },
                health: crate::runtime_types::ObservedHealth {
                    ready: false,
                    ..Default::default()
                },
                exit: Default::default(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploy_profile_deserializes_from_toml() {
        let toml_text = r#"
version = 1

[profiles.local-default]
backend = "local-process"

[profiles.homelab-container]
backend = "oci-container"
image = "ghcr.io/styrene-lab/omegon:v0.15.25"
namespace = "auspex"
restart_on_exit = true
omegon_flags = ["--strict-port", "--model", "anthropic:claude-haiku"]

[profiles.homelab-container.resources]
cpu = "1"
memory = "2Gi"

[profiles.homelab-container.health_check]
interval_secs = 15
failure_threshold = 5
initial_delay_secs = 20

[profiles.homelab-container.env]
OMEGON_LOG = "info"
RUST_BACKTRACE = "1"

[profiles.k8s-worker]
backend = "kubernetes"
image = "ghcr.io/styrene-lab/omegon:v0.15.25"
namespace = "agents"
max_instances = 8
requires = ["kubectl", "helm"]

[profiles.k8s-worker.resources]
cpu = "500m"
memory = "1Gi"
"#;

        let file: DeployProfilesFile = toml::from_str(toml_text).unwrap();

        assert_eq!(file.version, 1);
        assert_eq!(file.profiles.len(), 3);

        let local = file.profiles.get("local-default").unwrap();
        assert_eq!(local.backend, "local-process");
        assert!(local.image.is_none());
        assert!(!local.restart_on_exit);
        assert!(local.requires.is_none());

        let homelab = file.profiles.get("homelab-container").unwrap();
        assert_eq!(homelab.backend, "oci-container");
        assert_eq!(
            homelab.image.as_deref(),
            Some("ghcr.io/styrene-lab/omegon:v0.15.25")
        );
        assert!(homelab.restart_on_exit);
        assert_eq!(homelab.resources.as_ref().unwrap().cpu.as_deref(), Some("1"));
        assert_eq!(
            homelab.resources.as_ref().unwrap().memory.as_deref(),
            Some("2Gi")
        );
        let hc = homelab.health_check.as_ref().unwrap();
        assert_eq!(hc.interval_secs, 15);
        assert_eq!(hc.failure_threshold, 5);
        assert_eq!(hc.initial_delay_secs, 20);
        let flags = homelab.omegon_flags.as_ref().unwrap();
        assert_eq!(flags, &["--strict-port", "--model", "anthropic:claude-haiku"]);
        let env = homelab.env.as_ref().unwrap();
        assert_eq!(env.get("OMEGON_LOG").unwrap(), "info");

        let k8s = file.profiles.get("k8s-worker").unwrap();
        assert_eq!(k8s.backend, "kubernetes");
        assert_eq!(k8s.max_instances, Some(8));
        assert_eq!(
            k8s.requires.as_deref(),
            Some(&["kubectl".to_string(), "helm".to_string()][..])
        );
    }

    #[test]
    fn worker_profiles_deserialize_from_toml() {
        let toml_text = r#"
version = 1

[profiles.primary-interactive]
role = "primary-driver"
preferred_models = ["anthropic:claude-sonnet-4-6"]
thinking_level = "medium"
context_class = "clan"
tool_policy = "full"
memory_mode = "full"
max_runtime_seconds = 0
max_cost_usd = 0.0

[profiles.background-service]
role = "detached-service"
preferred_models = ["anthropic:claude-haiku"]
fallback_models = ["local:qwen2.5-coder"]
thinking_level = "minimal"
context_class = "maniple"
tool_policy = "bounded"
memory_mode = "project-only"
max_runtime_seconds = 0
max_cost_usd = 5.0
"#;

        let file: WorkerProfilesFile = toml::from_str(toml_text).unwrap();

        assert_eq!(file.version, 1);
        assert_eq!(file.profiles.len(), 2);

        let bg = file.profiles.get("background-service").unwrap();
        assert_eq!(
            bg.role,
            crate::runtime_types::WorkerRole::DetachedService
        );
        assert_eq!(bg.thinking_level, crate::runtime_types::ThinkingLevel::Minimal);
        assert_eq!(bg.memory_mode, crate::runtime_types::MemoryMode::ProjectOnly);
        assert_eq!(bg.max_cost_usd, 5.0);
    }

    #[test]
    fn resolved_config_lookups() {
        let mut config = ResolvedConfig::default();
        config.worker_profiles.profiles.insert(
            "test".into(),
            WorkerProfile {
                role: crate::runtime_types::WorkerRole::PrimaryDriver,
                thinking_level: crate::runtime_types::ThinkingLevel::High,
                context_class: "legion".into(),
                tool_policy: crate::runtime_types::ToolPolicy::Full,
                memory_mode: crate::runtime_types::MemoryMode::Full,
                ..Default::default()
            },
        );
        config.deploy_profiles.profiles.insert(
            "local".into(),
            DeployProfile {
                backend: "local-process".into(),
                ..Default::default()
            },
        );

        assert!(config.worker_profile("test").is_some());
        assert!(config.worker_profile("missing").is_none());
        assert!(config.deploy_profile("local").is_some());
        assert!(config.deploy_profile("missing").is_none());
    }

    #[test]
    fn required_tools_collected_across_profiles() {
        let mut config = ResolvedConfig::default();
        config.deploy_profiles.profiles.insert(
            "k8s".into(),
            DeployProfile {
                backend: "kubernetes".into(),
                requires: Some(vec!["kubectl".into(), "docker".into()]),
                ..Default::default()
            },
        );
        config.deploy_profiles.profiles.insert(
            "oci".into(),
            DeployProfile {
                backend: "oci-container".into(),
                requires: Some(vec!["docker".into()]),
                ..Default::default()
            },
        );
        config.deploy_profiles.profiles.insert(
            "local".into(),
            DeployProfile {
                backend: "local-process".into(),
                ..Default::default()
            },
        );

        let tools = config.required_tools();
        assert_eq!(tools, vec!["docker", "kubectl"]);
    }

    #[test]
    fn missing_tools_reports_absent_binaries() {
        let mut config = ResolvedConfig::default();
        config.deploy_profiles.profiles.insert(
            "k8s".into(),
            DeployProfile {
                backend: "kubernetes".into(),
                requires: Some(vec![
                    "auspex-test-nonexistent-binary-xyz".into(),
                ]),
                ..Default::default()
            },
        );

        let missing = config.missing_tools();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].0, "k8s");
        assert_eq!(missing[0].1, "auspex-test-nonexistent-binary-xyz");
    }

    #[test]
    fn available_deploy_profiles_excludes_unsatisfied() {
        let mut config = ResolvedConfig::default();
        config.deploy_profiles.profiles.insert(
            "local".into(),
            DeployProfile {
                backend: "local-process".into(),
                ..Default::default()
            },
        );
        config.deploy_profiles.profiles.insert(
            "needs-missing".into(),
            DeployProfile {
                backend: "kubernetes".into(),
                requires: Some(vec![
                    "auspex-test-nonexistent-binary-xyz".into(),
                ]),
                ..Default::default()
            },
        );

        let available = config.available_deploy_profiles();
        assert!(available.contains(&"local".to_string()));
        assert!(!available.contains(&"needs-missing".to_string()));
    }

    #[test]
    fn remote_instances_deserialize_from_toml() {
        let toml_text = r#"
version = 1

[instances.styrene-community-discord]
label = "Styrene Community Agent"
base_url = "https://agents.styrene.dev:7842"
role = "detached-service"
profile = "messaging-agent"
token_ref = "secret://auspex/instances/styrene-community-discord/token"
extensions = ["vox"]
tags = ["discord", "community"]

[instances.homelab-coding-agent]
label = "Homelab Coding Agent"
base_url = "http://192.168.1.50:7842"
"#;

        let file: RemoteInstancesFile = toml::from_str(toml_text).unwrap();

        assert_eq!(file.version, 1);
        assert_eq!(file.instances.len(), 2);

        let discord = file.instances.get("styrene-community-discord").unwrap();
        assert_eq!(discord.label, "Styrene Community Agent");
        assert_eq!(discord.base_url, "https://agents.styrene.dev:7842");
        assert_eq!(discord.role, WorkerRole::DetachedService);
        assert_eq!(discord.profile, "messaging-agent");
        assert_eq!(
            discord.token_ref.as_deref(),
            Some("secret://auspex/instances/styrene-community-discord/token")
        );
        assert_eq!(discord.extensions, vec!["vox"]);
        assert_eq!(discord.tags, vec!["discord", "community"]);
        assert!(discord.auto_attach);

        let homelab = file.instances.get("homelab-coding-agent").unwrap();
        assert_eq!(homelab.label, "Homelab Coding Agent");
        assert_eq!(homelab.role, WorkerRole::DetachedService); // default
        assert_eq!(homelab.profile, "remote-agent"); // default
        assert_eq!(homelab.auth_mode, "ephemeral-bearer"); // default
        assert!(homelab.auto_attach); // default
    }

    #[test]
    fn remote_instance_url_derivation() {
        let entry = RemoteInstanceEntry {
            label: "test".into(),
            base_url: "https://agents.styrene.dev:7842".into(),
            token_ref: Some("my-token".into()),
            ..Default::default()
        };

        assert_eq!(
            entry.startup_url(),
            "https://agents.styrene.dev:7842/api/startup"
        );
        assert_eq!(
            entry.health_url(),
            "https://agents.styrene.dev:7842/api/healthz"
        );
        assert_eq!(
            entry.ready_url(),
            "https://agents.styrene.dev:7842/api/readyz"
        );
        assert_eq!(
            entry.ws_url(),
            "wss://agents.styrene.dev:7842/ws?token=my-token"
        );
    }

    #[test]
    fn remote_instance_ws_url_without_token() {
        let entry = RemoteInstanceEntry {
            label: "test".into(),
            base_url: "http://192.168.1.50:7842".into(),
            ..Default::default()
        };

        assert_eq!(entry.ws_url(), "ws://192.168.1.50:7842/ws");
    }

    #[test]
    fn remote_instance_to_instance_record() {
        let entry = RemoteInstanceEntry {
            label: "Discord Agent".into(),
            base_url: "https://agents.styrene.dev:7842".into(),
            role: WorkerRole::DetachedService,
            profile: "messaging-agent".into(),
            token_ref: Some("secret://token".into()),
            extensions: vec!["vox".into()],
            tags: vec!["discord".into()],
            auth_mode: "ephemeral-bearer".into(),
            auto_attach: true,
        };

        let record = entry.to_instance_record("styrene-discord");

        assert_eq!(record.identity.instance_id, "remote:styrene-discord");
        assert_eq!(record.identity.role, WorkerRole::DetachedService);
        assert_eq!(record.identity.profile, "messaging-agent");
        assert_eq!(
            record.ownership.owner_kind,
            crate::runtime_types::OwnerKind::External
        );
        assert_eq!(
            record.observed.control_plane.base_url,
            "https://agents.styrene.dev:7842"
        );
        assert_eq!(
            record.observed.control_plane.startup_url,
            "https://agents.styrene.dev:7842/api/startup"
        );
        assert!(record
            .observed
            .control_plane
            .ws_url
            .contains("wss://"));
        assert!(record
            .observed
            .control_plane
            .ws_url
            .contains("token=secret://token"));
        assert_eq!(
            record.observed.control_plane.token_ref.as_deref(),
            Some("secret://token")
        );
        assert!(!record.observed.health.ready);
    }

    #[test]
    fn auto_attach_instances_filters_correctly() {
        let mut config = ResolvedConfig::default();
        config.remote_instances.instances.insert(
            "auto".into(),
            RemoteInstanceEntry {
                label: "Auto".into(),
                base_url: "http://host1:7842".into(),
                auto_attach: true,
                ..Default::default()
            },
        );
        config.remote_instances.instances.insert(
            "manual".into(),
            RemoteInstanceEntry {
                label: "Manual".into(),
                base_url: "http://host2:7842".into(),
                auto_attach: false,
                ..Default::default()
            },
        );

        let auto = config.auto_attach_instances();
        assert_eq!(auto.len(), 1);
        assert_eq!(auto[0].0, "auto");
    }
}
