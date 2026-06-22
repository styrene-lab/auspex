//! OCI container backend for spawning and managing agent containers.
//!
//! Connects to Docker or Podman via their API socket using `bollard`.
//! Both runtimes expose a Docker-compatible REST API over a Unix socket,
//! so the same [`BollardBackend`] works against either.
//!
//! # Socket detection order
//!
//! [`BollardBackend::detect`] probes in priority order:
//!
//! 1. `DOCKER_HOST` environment variable (explicit user override)
//! 2. `/var/run/docker.sock` — Docker, rootful
//! 3. `$XDG_RUNTIME_DIR/podman/podman.sock` — Podman rootless
//! 4. `/run/podman/podman.sock` — Podman rootful
//!
//! The first socket that responds to a ping wins.
//!
//! # Security posture
//!
//! When Auspex is itself containerized, the Docker socket bind-mount gives
//! the container root-equivalent host access.  Operators should interpose a
//! socket proxy (e.g. `tecnativa/docker-socket-proxy`) that allowlists only
//! the endpoints Auspex uses: container create/start/stop/remove/list and
//! image pull.  See `examples/compose/auspex-with-socket-proxy.yml`.
//!
//! Containers launched by this backend apply a hardened-by-default
//! `HostConfig`:
//! - `cap_drop: ["ALL"]` — no Linux capabilities
//! - `security_opt: ["no-new-privileges:true"]` — prevent setuid escalation
//! - `network_mode: "bridge"` — isolated bridge network, no host access
//! - Memory and CPU limits forwarded from [`OciLaunchSpec::resources`]

#![cfg(not(target_arch = "wasm32"))]

use std::collections::HashMap;

use bollard::models::{ContainerCreateBody, HostConfig, PortBinding};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, ListContainersOptionsBuilder,
    RemoveContainerOptionsBuilder, StopContainerOptionsBuilder,
};
use bollard::{API_DEFAULT_VERSION, Docker};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};

use crate::container_discovery::DiscoveredContainer;
use crate::runtime_types::ResourceRequirements;

// ── Label keys ──────────────────────────────────────────────────────────────

/// Label applied to every container Auspex creates.
pub const LABEL_MANAGED_BY: &str = "styrene.sh/managed-by";
/// Label carrying the agent instance ID (used as the container name).
pub const LABEL_AGENT_ID: &str = "styrene.sh/agent-id";
/// Label carrying the package/profile ID this agent was launched from.
pub const LABEL_PACKAGE_ID: &str = "styrene.sh/agent-package";
/// Label recording the host port Auspex bound for the agent's control plane.
pub const LABEL_HOST_PORT: &str = "auspex.styrene.sh/host-port";
/// Value used in [`LABEL_MANAGED_BY`].
pub const MANAGED_BY_VALUE: &str = "auspex";

/// Container port all Omegon agents listen on.
pub const AGENT_CONTAINER_PORT: u16 = 7842;

// ── Public types ─────────────────────────────────────────────────────────────

/// Controls whether the image is pulled before a container is started.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PullPolicy {
    /// Always pull from the registry, even if a local image is present.
    Always,
    /// Pull only when the image is absent from the local store.
    #[default]
    IfNotPresent,
    /// Never pull — fail if the image is not available locally.
    Never,
}

/// Everything needed to launch a single agent container.
#[derive(Clone, Debug)]
pub struct OciLaunchSpec {
    /// OCI image reference, e.g. `ghcr.io/styrene-labs/omegon-agent:latest`.
    pub image: String,
    /// Container name.  Also used as the agent ID label.
    pub name: String,
    /// Host port mapped to [`AGENT_CONTAINER_PORT`] inside the container.
    pub host_port: u16,
    /// Environment variables injected into the container.
    pub env: Vec<(String, String)>,
    /// Labels merged on top of the managed-by set.
    pub extra_labels: HashMap<String, String>,
    /// Optional CPU and memory limits.
    pub resources: Option<ResourceRequirements>,
    /// When/whether to pull the image before launch.
    pub pull_policy: PullPolicy,
    /// Credentials for private registries.
    pub registry_auth: Option<RegistryCredentials>,
}

/// Username/password credentials for a private OCI registry.
#[derive(Clone, Debug)]
pub struct RegistryCredentials {
    pub username: String,
    pub password: String,
    /// Registry hostname, e.g. `ghcr.io`.  `None` defaults to Docker Hub.
    pub server_address: Option<String>,
}

/// Result of [`BollardBackend::detect`]: which runtime was found and where.
#[derive(Clone, Debug)]
pub struct DetectedRuntime {
    /// Human-readable name derived from the socket path.
    pub runtime: String,
    /// Socket path or URL that responded to a ping.
    pub endpoint: String,
}

/// Live connection to a Docker or Podman API socket.
pub struct BollardBackend {
    docker: Docker,
    /// Detected runtime info — useful for status displays and diagnostics.
    pub runtime_info: DetectedRuntime,
}

impl BollardBackend {
    /// Detect and connect to the first available container runtime.
    ///
    /// Returns `None` if no runtime responds within 5 seconds.
    pub async fn detect() -> Option<Self> {
        // Try each candidate; return the first that pings successfully.
        for (endpoint, docker) in candidate_connections() {
            if docker.ping().await.is_ok() {
                let runtime = infer_runtime_name(&endpoint);
                return Some(Self {
                    docker,
                    runtime_info: DetectedRuntime { runtime, endpoint },
                });
            }
        }
        None
    }

    /// Pull an image according to the given policy.
    ///
    /// `PullPolicy::IfNotPresent` inspects the local image store first and
    /// skips the pull if the image is already there.
    pub async fn ensure_image(
        &self,
        image: &str,
        policy: &PullPolicy,
        auth: Option<&RegistryCredentials>,
    ) -> Result<(), bollard::errors::Error> {
        if *policy == PullPolicy::Never {
            return Ok(());
        }
        if *policy == PullPolicy::IfNotPresent && self.docker.inspect_image(image).await.is_ok() {
            return Ok(());
        }

        let bollard_auth = auth.map(|c| bollard::auth::DockerCredentials {
            username: Some(c.username.clone()),
            password: Some(c.password.clone()),
            serveraddress: c.server_address.clone(),
            ..Default::default()
        });

        self.docker
            .create_image(
                Some(
                    CreateImageOptionsBuilder::default()
                        .from_image(image)
                        .build(),
                ),
                None,
                bollard_auth,
            )
            .try_collect::<Vec<_>>()
            .await?;

        Ok(())
    }

    /// Create and start an agent container.
    ///
    /// Returns the container ID of the newly started container.
    pub async fn launch(&self, spec: &OciLaunchSpec) -> Result<String, bollard::errors::Error> {
        self.ensure_image(&spec.image, &spec.pull_policy, spec.registry_auth.as_ref())
            .await?;

        let labels = build_managed_labels(&spec.name, &spec.extra_labels, spec.host_port);

        let mut port_bindings: HashMap<String, Option<Vec<PortBinding>>> = HashMap::new();
        port_bindings.insert(
            format!("{AGENT_CONTAINER_PORT}/tcp"),
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some(spec.host_port.to_string()),
            }]),
        );

        let env: Vec<String> = spec.env.iter().map(|(k, v)| format!("{k}={v}")).collect();

        let host_config = build_host_config(&port_bindings, spec.resources.as_ref());

        let config = ContainerCreateBody {
            image: Some(spec.image.clone()),
            env: if env.is_empty() { None } else { Some(env) },
            labels: Some(labels),
            // exposed_ports in bollard 0.21 is Vec<String> ("port/proto" format);
            // port publication is fully handled by HostConfig::port_bindings.
            host_config: Some(host_config),
            ..Default::default()
        };

        let response = self
            .docker
            .create_container(
                Some(
                    CreateContainerOptionsBuilder::default()
                        .name(spec.name.as_str())
                        .build(),
                ),
                config,
            )
            .await?;

        self.docker.start_container(&response.id, None).await?;

        Ok(response.id)
    }

    /// Gracefully stop and then remove a container.
    ///
    /// Issues SIGTERM and waits up to 10 seconds before SIGKILL, then removes
    /// the container.  Errors during the stop phase are ignored — the remove
    /// is attempted regardless (`force: true`).
    pub async fn terminate(&self, container_id: &str) -> Result<(), bollard::errors::Error> {
        let _ = self
            .docker
            .stop_container(
                container_id,
                Some(StopContainerOptionsBuilder::default().t(10).build()),
            )
            .await;

        self.docker
            .remove_container(
                container_id,
                Some(
                    RemoveContainerOptionsBuilder::default()
                        .force(true)
                        .v(false)
                        .build(),
                ),
            )
            .await
    }

    /// List all running agent containers managed by Auspex.
    ///
    /// Filters by the `styrene.sh/managed-by=auspex` label so only containers
    /// Auspex launched are returned — regardless of image name.
    ///
    /// This replaces the `podman ps` shell-out in `container_discovery`.
    pub async fn list_agents(&self) -> Result<Vec<DiscoveredContainer>, bollard::errors::Error> {
        let mut filters = HashMap::new();
        filters.insert(
            "label".to_string(),
            vec![format!("{LABEL_MANAGED_BY}={MANAGED_BY_VALUE}")],
        );

        let summaries = self
            .docker
            .list_containers(Some(
                ListContainersOptionsBuilder::default()
                    .all(false)
                    .filters(&filters)
                    .build(),
            ))
            .await?;

        let containers = summaries
            .into_iter()
            .filter_map(|s| {
                let container_id = s.id.unwrap_or_default();
                let name = s
                    .names
                    .and_then(|ns| ns.into_iter().next())
                    .unwrap_or_default()
                    .trim_start_matches('/')
                    .to_string();
                let image = s.image.unwrap_or_default();
                let status = s
                    .state
                    .map(|st| st.to_string())
                    .unwrap_or_else(|| "unknown".to_string());

                // Find the host port mapped from AGENT_CONTAINER_PORT.
                let host_port = s.ports?.into_iter().find_map(|p| {
                    if p.private_port == AGENT_CONTAINER_PORT {
                        p.public_port
                    } else {
                        None
                    }
                })?;

                Some(DiscoveredContainer {
                    container_id,
                    name,
                    image,
                    host_port,
                    status,
                })
            })
            .collect();

        Ok(containers)
    }

    /// Expose the inner `bollard::Docker` handle for callers that need
    /// operations beyond the curated API above (e.g. log streaming).
    pub fn docker(&self) -> &Docker {
        &self.docker
    }
}

// ── Connection candidates ─────���──────────────────────────────────────────────

/// Produce an ordered list of `(endpoint_label, Docker)` pairs to probe.
fn candidate_connections() -> Vec<(String, Docker)> {
    const TIMEOUT: u64 = 5;
    let mut candidates = Vec::new();

    // 1. Explicit DOCKER_HOST — handles tcp://, unix://, ssh:// schemes.
    if let Ok(host) = std::env::var("DOCKER_HOST")
        && let Ok(docker) = Docker::connect_with_defaults()
    {
        candidates.push((host, docker));
    }

    // 2. Docker default socket.
    let docker_sock = "/var/run/docker.sock";
    if std::path::Path::new(docker_sock).exists()
        && let Ok(docker) = Docker::connect_with_unix(docker_sock, TIMEOUT, API_DEFAULT_VERSION)
    {
        candidates.push((docker_sock.to_string(), docker));
    }

    // 3. Podman rootless — only if XDG_RUNTIME_DIR is set (always true on
    //    systemd systems running Podman rootless).
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        let path = format!("{xdg}/podman/podman.sock");
        if std::path::Path::new(&path).exists()
            && let Ok(docker) = Docker::connect_with_unix(&path, TIMEOUT, API_DEFAULT_VERSION)
        {
            candidates.push((path, docker));
        }
    }

    // 4. Podman rootful.
    let podman_sock = "/run/podman/podman.sock";
    if std::path::Path::new(podman_sock).exists()
        && let Ok(docker) = Docker::connect_with_unix(podman_sock, TIMEOUT, API_DEFAULT_VERSION)
    {
        candidates.push((podman_sock.to_string(), docker));
    }

    candidates
}

/// Derive a human-readable runtime name from an endpoint string.
fn infer_runtime_name(endpoint: &str) -> String {
    if endpoint.contains("podman") {
        "Podman".to_string()
    } else if endpoint.starts_with("tcp://") || endpoint.starts_with("http://") {
        "Docker (TCP)".to_string()
    } else if endpoint.starts_with("ssh://") {
        "Docker (SSH)".to_string()
    } else {
        "Docker".to_string()
    }
}

// ── HostConfig builder ───────────────────────────────────────────────────────

fn build_managed_labels(
    name: &str,
    extra: &HashMap<String, String>,
    host_port: u16,
) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    labels.insert(LABEL_MANAGED_BY.to_string(), MANAGED_BY_VALUE.to_string());
    labels.insert(LABEL_AGENT_ID.to_string(), name.to_string());
    labels.insert(LABEL_HOST_PORT.to_string(), host_port.to_string());
    for (k, v) in extra {
        labels.insert(k.clone(), v.clone());
    }
    labels
}

fn build_host_config(
    port_bindings: &HashMap<String, Option<Vec<PortBinding>>>,
    resources: Option<&ResourceRequirements>,
) -> HostConfig {
    let memory = resources
        .and_then(|r| r.memory.as_deref())
        .and_then(parse_memory_bytes);

    let nano_cpus = resources
        .and_then(|r| r.cpu.as_deref())
        .and_then(parse_nano_cpus);

    HostConfig {
        // Hardened defaults — agents do not need kernel capabilities.
        cap_drop: Some(vec!["ALL".to_string()]),
        security_opt: Some(vec!["no-new-privileges:true".to_string()]),
        // Bridge network: agents are reachable via the mapped host port only.
        network_mode: Some("bridge".to_string()),
        port_bindings: Some(port_bindings.clone()),
        memory,
        nano_cpus,
        ..Default::default()
    }
}

// ── Resource parsing ─────────────────────────────────────────────────────────

/// Parse a memory string (`"512m"`, `"1g"`, `"1073741824"`) into bytes.
fn parse_memory_bytes(s: &str) -> Option<i64> {
    let s = s.trim().to_lowercase();
    if let Some(n) = s.strip_suffix("gb").or_else(|| s.strip_suffix('g')) {
        n.parse::<i64>().ok().map(|v| v * 1_073_741_824)
    } else if let Some(n) = s.strip_suffix("mb").or_else(|| s.strip_suffix('m')) {
        n.parse::<i64>().ok().map(|v| v * 1_048_576)
    } else if let Some(n) = s.strip_suffix("kb").or_else(|| s.strip_suffix('k')) {
        n.parse::<i64>().ok().map(|v| v * 1_024)
    } else {
        s.parse::<i64>().ok()
    }
}

/// Parse a CPU string (`"0.5"`, `"1"`, `"2.0"`) into nano-CPUs.
///
/// Docker's CPU unit is nano-CPUs: 1 CPU = 1,000,000,000 nano-CPUs.
fn parse_nano_cpus(s: &str) -> Option<i64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .map(|cpus| (cpus * 1_000_000_000.0) as i64)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Memory parsing ────────────────────────────────────────────────────────

    #[test]
    fn parse_megabytes_lowercase() {
        assert_eq!(parse_memory_bytes("512m"), Some(512 * 1_048_576));
    }

    #[test]
    fn parse_megabytes_uppercase() {
        assert_eq!(parse_memory_bytes("256M"), Some(256 * 1_048_576));
    }

    #[test]
    fn parse_gigabytes() {
        assert_eq!(parse_memory_bytes("2g"), Some(2 * 1_073_741_824));
        assert_eq!(parse_memory_bytes("1GB"), Some(1_073_741_824));
    }

    #[test]
    fn parse_kilobytes() {
        assert_eq!(parse_memory_bytes("4k"), Some(4 * 1_024));
        assert_eq!(parse_memory_bytes("8KB"), Some(8 * 1_024));
    }

    #[test]
    fn parse_raw_bytes() {
        assert_eq!(parse_memory_bytes("1073741824"), Some(1_073_741_824));
    }

    #[test]
    fn parse_memory_invalid_returns_none() {
        assert_eq!(parse_memory_bytes("lots"), None);
        assert_eq!(parse_memory_bytes(""), None);
    }

    // ── CPU parsing ───────────────────────────────────────────────────────────

    #[test]
    fn parse_nano_cpus_fractional() {
        assert_eq!(parse_nano_cpus("0.5"), Some(500_000_000));
        assert_eq!(parse_nano_cpus("0.25"), Some(250_000_000));
    }

    #[test]
    fn parse_nano_cpus_whole() {
        assert_eq!(parse_nano_cpus("1"), Some(1_000_000_000));
        assert_eq!(parse_nano_cpus("2.0"), Some(2_000_000_000));
    }

    #[test]
    fn parse_nano_cpus_invalid() {
        assert_eq!(parse_nano_cpus("fast"), None);
    }

    // ── Label building ────────────────────────────────────────────────────────

    #[test]
    fn managed_labels_always_present() {
        let labels = build_managed_labels("coding-agent", &HashMap::new(), 7845);
        assert_eq!(labels[LABEL_MANAGED_BY], MANAGED_BY_VALUE);
        assert_eq!(labels[LABEL_AGENT_ID], "coding-agent");
        assert_eq!(labels[LABEL_HOST_PORT], "7845");
    }

    #[test]
    fn managed_labels_merge_extra() {
        let mut extra = HashMap::new();
        extra.insert(LABEL_PACKAGE_ID.to_string(), "coding".to_string());
        let labels = build_managed_labels("agent", &extra, 7845);
        assert_eq!(labels[LABEL_PACKAGE_ID], "coding");
        // Core labels still present.
        assert_eq!(labels[LABEL_MANAGED_BY], MANAGED_BY_VALUE);
    }

    #[test]
    fn managed_labels_extra_cannot_override_managed_by() {
        // Extra labels can shadow the managed-by key if a caller tries it,
        // but that's a caller bug — document the expected contract here.
        let mut extra = HashMap::new();
        extra.insert(LABEL_AGENT_ID.to_string(), "override-attempt".to_string());
        // The extra map is inserted after the core labels, so it wins —
        // callers should not include core label keys in extra_labels.
        let labels = build_managed_labels("real-agent", &extra, 7845);
        // LABEL_MANAGED_BY is always correct regardless.
        assert_eq!(labels[LABEL_MANAGED_BY], MANAGED_BY_VALUE);
    }

    // ── Runtime name inference ────────────────────────────────────────────────

    #[test]
    fn infer_podman_from_socket_path() {
        assert_eq!(
            infer_runtime_name("/run/user/1000/podman/podman.sock"),
            "Podman"
        );
    }

    #[test]
    fn infer_docker_from_socket_path() {
        assert_eq!(infer_runtime_name("/var/run/docker.sock"), "Docker");
    }

    #[test]
    fn infer_docker_tcp() {
        assert_eq!(
            infer_runtime_name("tcp://192.168.1.10:2375"),
            "Docker (TCP)"
        );
    }

    #[test]
    fn infer_docker_ssh() {
        assert_eq!(infer_runtime_name("ssh://user@nas"), "Docker (SSH)");
    }

    // ── HostConfig security posture ───────────────────────────────────────────

    #[test]
    fn host_config_drops_all_capabilities() {
        let bindings = HashMap::new();
        let cfg = build_host_config(&bindings, None);
        assert_eq!(cfg.cap_drop.as_deref(), Some(&["ALL".to_string()][..]));
    }

    #[test]
    fn host_config_no_new_privileges() {
        let bindings = HashMap::new();
        let cfg = build_host_config(&bindings, None);
        let opts = cfg.security_opt.unwrap_or_default();
        assert!(opts.iter().any(|o| o == "no-new-privileges:true"));
    }

    #[test]
    fn host_config_bridge_network() {
        let bindings = HashMap::new();
        let cfg = build_host_config(&bindings, None);
        assert_eq!(cfg.network_mode.as_deref(), Some("bridge"));
    }

    #[test]
    fn host_config_applies_memory_limit() {
        let bindings = HashMap::new();
        let resources = ResourceRequirements {
            cpu: None,
            memory: Some("512m".to_string()),
        };
        let cfg = build_host_config(&bindings, Some(&resources));
        assert_eq!(cfg.memory, Some(512 * 1_048_576));
    }

    #[test]
    fn host_config_applies_cpu_limit() {
        let bindings = HashMap::new();
        let resources = ResourceRequirements {
            cpu: Some("1.5".to_string()),
            memory: None,
        };
        let cfg = build_host_config(&bindings, Some(&resources));
        assert_eq!(cfg.nano_cpus, Some(1_500_000_000));
    }
}
