//! Container discovery for podman/docker-managed omegon agents.
//!
//! Scans `podman ps` for running containers from the `auspex-agents` image,
//! probes their health endpoints, and produces `InstanceRecord`s for the
//! instance registry.

use crate::capability_registry::InstanceCapabilitySnapshot;
use crate::compatibility::assess_observed_control_plane;
use crate::runtime_types::{
    BackendConfig, BackendKind, DesiredWorkerState, InstanceRecord, ObservedControlPlane,
    ObservedHealth, ObservedPlacement, ObservedWorkerState, OwnerKind, PolicyOverrides,
    WorkerIdentity, WorkerLifecycleState, WorkerOwnership, WorkerRole, WorkspaceBinding,
};

/// A running container discovered via `podman ps`.
#[derive(Clone, Debug)]
pub struct DiscoveredContainer {
    pub container_id: String,
    pub name: String,
    pub image: String,
    pub host_port: u16,
    pub status: String,
}

/// Discover running Docker/Podman containers managed by Auspex.
///
/// The primary path uses the native bollard client against Docker or Podman
/// sockets and filters by `styrene.sh/managed-by=auspex`.  If no compatible
/// socket is available, this falls back to the legacy `podman ps` shell-out
/// for older hand-launched `auspex-agents` containers.
#[cfg(not(target_arch = "wasm32"))]
pub fn discover_containers() -> Vec<DiscoveredContainer> {
    discover_containers_via_bollard().unwrap_or_else(discover_containers_via_podman_cli)
}

/// Native Docker/Podman discovery through bollard.
#[cfg(not(target_arch = "wasm32"))]
fn discover_containers_via_bollard() -> Option<Vec<DiscoveredContainer>> {
    if tokio::runtime::Handle::try_current().is_ok() {
        return None;
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()?;

    runtime.block_on(async {
        let backend = crate::oci_backend::BollardBackend::detect().await?;
        backend.list_agents().await.ok()
    })
}

/// Legacy fallback for pre-bollard local Podman agents.
#[cfg(not(target_arch = "wasm32"))]
fn discover_containers_via_podman_cli() -> Vec<DiscoveredContainer> {
    let output = match std::process::Command::new("podman")
        .args(["ps", "--format", "json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return vec![],
    };

    let json_str = String::from_utf8_lossy(&output.stdout);
    let containers: Vec<serde_json::Value> = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    containers
        .into_iter()
        .filter_map(container_from_podman_json)
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn container_from_podman_json(container: serde_json::Value) -> Option<DiscoveredContainer> {
    let image = container.get("Image")?.as_str()?.to_string();
    if !image.contains("auspex-agents") {
        return None;
    }

    let container_id = container
        .get("Id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let name = container
        .get("Names")
        .and_then(|v| {
            v.as_array()
                .and_then(|a| a.first())
                .and_then(|n| n.as_str())
                .or_else(|| v.as_str())
        })
        .unwrap_or("")
        .to_string();

    let host_port = extract_host_port(&container)?;

    let status = container
        .get("State")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    Some(DiscoveredContainer {
        container_id,
        name,
        image,
        host_port,
        status,
    })
}

/// Extract the host port mapped to container port 7842.
fn extract_host_port(container: &serde_json::Value) -> Option<u16> {
    // podman ps --format json puts port info in "Ports" array
    let ports = container.get("Ports")?.as_array()?;
    for port in ports {
        let container_port = port
            .get("container_port")
            .or_else(|| port.get("containerPort"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if container_port == 7842 {
            return port
                .get("host_port")
                .or_else(|| port.get("hostPort"))
                .and_then(|v| v.as_u64())
                .map(|p| p as u16);
        }
    }
    None
}

/// Probe a container's health endpoint synchronously.
/// Returns true if the container reports ready.
#[cfg(not(target_arch = "wasm32"))]
pub fn probe_health(host_port: u16) -> bool {
    let url = format!("http://127.0.0.1:{host_port}/api/readyz");
    let output = std::process::Command::new("curl")
        .args(["-sf", "--max-time", "2", &url])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let body = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v.get("ok").and_then(|ok| ok.as_bool()))
                .unwrap_or(false)
        }
        _ => false,
    }
}

fn parse_startup_info(body: &str) -> Option<(String, String, Vec<String>)> {
    let startup: serde_json::Value = serde_json::from_str(body).ok()?;

    let token = startup
        .get("token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let omegon_version = startup
        .pointer("/instance_descriptor/control_plane/omegon_version")
        .or_else(|| startup.pointer("/instance_descriptor/identity/omegon_version"))
        .or_else(|| startup.get("omegon_version"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let capabilities = startup
        .pointer("/instance_descriptor/control_plane/capabilities")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some((token, omegon_version, capabilities))
}

/// Fetch the omegon version from a container's startup endpoint.
#[cfg(not(target_arch = "wasm32"))]
fn fetch_startup_info(host_port: u16) -> Option<(String, String, Vec<String>)> {
    let url = format!("http://127.0.0.1:{host_port}/api/startup");
    let output = std::process::Command::new("curl")
        .args(["-sf", "--max-time", "2", &url])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let body = String::from_utf8_lossy(&output.stdout);
    parse_startup_info(&body)
}

/// Convert a discovered container into an `InstanceRecord`.
#[cfg(not(target_arch = "wasm32"))]
pub fn container_to_instance_record(container: &DiscoveredContainer) -> InstanceRecord {
    let ready = probe_health(container.host_port);
    let (token, omegon_version, capabilities) =
        fetch_startup_info(container.host_port).unwrap_or_default();

    let base_url = format!("http://127.0.0.1:{}", container.host_port);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();

    InstanceRecord {
        schema_version: 1,
        identity: WorkerIdentity {
            instance_id: format!("container:{}", container.name),
            role: WorkerRole::DetachedService,
            profile: "container-agent".into(),
            raw_role: None,
            raw_profile: None,
            raw_runtime_profile: None,
            status: if ready {
                WorkerLifecycleState::Ready
            } else {
                WorkerLifecycleState::Starting
            },
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        ownership: WorkerOwnership {
            owner_kind: OwnerKind::AuspexSession,
            owner_id: "auspex".into(),
            parent_instance_id: None,
        },
        desired: DesiredWorkerState {
            backend: BackendConfig {
                kind: BackendKind::OciContainer,
                image: Some(container.image.clone()),
                ..Default::default()
            },
            workspace: WorkspaceBinding {
                cwd: "/workspace".into(),
                workspace_id: format!("container:{}", container.name),
                ..Default::default()
            },
            policy: PolicyOverrides::default(),
            task: None,
            security: Default::default(),
        },
        observed: {
            let control_plane = ObservedControlPlane {
                schema_version: 2,
                omegon_version,
                base_url: base_url.clone(),
                startup_url: format!("{base_url}/api/startup"),
                health_url: format!("{base_url}/api/healthz"),
                ready_url: format!("{base_url}/api/readyz"),
                ws_url: format!(
                    "ws://127.0.0.1:{}/ws{}",
                    container.host_port,
                    if token.is_empty() {
                        String::new()
                    } else {
                        format!("?token={token}")
                    }
                ),
                acp_url: Some(format!(
                    "ws://127.0.0.1:{}/acp{}",
                    container.host_port,
                    if token.is_empty() {
                        String::new()
                    } else {
                        format!("?token={token}")
                    }
                )),
                auth_mode: "ephemeral-bearer".into(),
                token_ref: if token.is_empty() { None } else { Some(token) },
                transport_security: Some("plaintext".into()),
                mtls: Some(false),
                last_ready_at: if ready { Some(now.clone()) } else { None },
                ..Default::default()
            };
            ObservedWorkerState {
                placement: ObservedPlacement {
                    placement_id: container.container_id.clone(),
                    host: "localhost".into(),
                    container_name: Some(container.name.clone()),
                    ..Default::default()
                },
                control_plane: control_plane.clone(),
                health: ObservedHealth {
                    ready,
                    last_heartbeat_at: Some(now.clone()),
                    last_seen_at: Some(now),
                    ..Default::default()
                },
                exit: Default::default(),
                compatibility: Some(assess_observed_control_plane(&control_plane)),
                operational_profile: None,
                capabilities: Some(
                    InstanceCapabilitySnapshot::from_instance_descriptor_capabilities(
                        format!("container:{}", container.name),
                        capabilities,
                    ),
                ),
            }
        },
    }
}

/// Discover all running auspex-agent containers, probe their health,
/// and return instance records for each.
#[cfg(not(target_arch = "wasm32"))]
pub fn discover_and_probe() -> Vec<InstanceRecord> {
    discover_containers()
        .iter()
        .map(container_to_instance_record)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_host_port_from_podman_json() {
        let container = serde_json::json!({
            "Id": "abc123",
            "Names": ["coding-agent"],
            "Image": "localhost/auspex-agents:latest",
            "State": "running",
            "Ports": [
                {
                    "host_ip": "",
                    "container_port": 7842,
                    "host_port": 7845,
                    "range": 1,
                    "protocol": "tcp"
                }
            ]
        });

        assert_eq!(extract_host_port(&container), Some(7845));
    }

    #[test]
    fn extract_host_port_returns_none_for_no_matching_port() {
        let container = serde_json::json!({
            "Ports": [
                { "container_port": 8080, "host_port": 9090 }
            ]
        });

        assert_eq!(extract_host_port(&container), None);
    }

    #[test]
    fn parse_startup_info_prefers_control_plane_version_and_capabilities() {
        let body = r#"{
            "token": "secret-token",
            "omegon_version": "0.24.0",
            "instance_descriptor": {
                "identity": { "omegon_version": "0.25.0" },
                "control_plane": {
                    "omegon_version": "0.25.4",
                    "capabilities": ["state.snapshot", "events.stream", 42]
                }
            }
        }"#;

        let (token, version, capabilities) = parse_startup_info(body).unwrap();

        assert_eq!(token, "secret-token");
        assert_eq!(version, "0.25.4");
        assert_eq!(capabilities, vec!["state.snapshot", "events.stream"]);
    }

    #[test]
    fn parse_startup_info_falls_back_to_top_level_version() {
        let body = r#"{ "omegon_version": "0.25.4" }"#;

        let (_token, version, capabilities) = parse_startup_info(body).unwrap();

        assert_eq!(version, "0.25.4");
        assert!(capabilities.is_empty());
    }

    #[test]
    fn container_to_instance_record_produces_valid_record() {
        let container = DiscoveredContainer {
            container_id: "abc123def456".into(),
            name: "slack-agent".into(),
            image: "localhost/auspex-agents:latest".into(),
            host_port: 7843,
            status: "running".into(),
        };

        // This won't probe (no container running), but the record structure is valid.
        let record = container_to_instance_record(&container);

        assert_eq!(record.identity.instance_id, "container:slack-agent");
        assert_eq!(record.identity.role, WorkerRole::DetachedService);
        assert_eq!(record.desired.backend.kind, BackendKind::OciContainer);
        assert_eq!(
            record.observed.control_plane.base_url,
            "http://127.0.0.1:7843"
        );
        assert_eq!(record.observed.placement.placement_id, "abc123def456");
        assert_eq!(
            record.observed.placement.container_name.as_deref(),
            Some("slack-agent")
        );
    }
}
