#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::runtime_types::InstanceRecord;

const INSTANCE_REGISTRY_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceRegistryStore {
    pub schema_version: u32,
    #[serde(default)]
    pub instances: Vec<InstanceRecord>,
}

impl Default for InstanceRegistryStore {
    fn default() -> Self {
        Self {
            schema_version: INSTANCE_REGISTRY_SCHEMA_VERSION,
            instances: Vec::new(),
        }
    }
}

impl InstanceRegistryStore {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn default_instance_registry_path() -> Option<std::path::PathBuf> {
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                let mut path = std::path::PathBuf::from(home);
                path.push(".config");
                path
            })
        })?;
    let mut path = config_root;
    path.push("auspex");
    path.push("instance-registry.json");
    Some(path)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_or_default(path: &std::path::Path) -> InstanceRegistryStore {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|json| InstanceRegistryStore::from_json(&json).ok())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn persist(path: &std::path::Path, store: &InstanceRegistryStore) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = store
        .to_json_pretty()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::runtime_types::{
        BackendConfig, BackendKind, InstanceRecord, ObservedControlPlane, ObservedExit,
        ObservedHealth, ObservedPlacement, ObservedWorkerState, OwnerKind, PolicyOverrides,
        WorkerIdentity, WorkerLifecycleState, WorkerOwnership, WorkerRole, WorkspaceBinding,
    };

    fn sample_record(instance_id: &str) -> InstanceRecord {
        InstanceRecord {
            schema_version: 1,
            identity: WorkerIdentity {
                instance_id: instance_id.into(),
                role: WorkerRole::SupervisedChild,
                profile: "cheap-subtask".into(),
                status: WorkerLifecycleState::Ready,
                created_at: "2026-04-03T12:00:00Z".into(),
                updated_at: "2026-04-03T12:03:42Z".into(),
            },
            ownership: WorkerOwnership {
                owner_kind: OwnerKind::AuspexSession,
                owner_id: "session_01HV".into(),
                parent_instance_id: Some("omg_primary_01HV".into()),
            },
            desired: crate::runtime_types::DesiredWorkerState {
                backend: BackendConfig {
                    kind: BackendKind::LocalProcess,
                    image: None,
                    namespace: None,
                    resources: Default::default(),
                },
                workspace: WorkspaceBinding {
                    cwd: "/repo/path".into(),
                    workspace_id: "repo:8f2f4c1".into(),
                    branch: Some("main".into()),
                },
                task: None,
                policy: PolicyOverrides {
                    model: Some("anthropic:claude-haiku".into()),
                    ..Default::default()
                },
            },
            observed: ObservedWorkerState {
                placement: ObservedPlacement {
                    placement_id: format!("pid:{instance_id}"),
                    host: "localhost".into(),
                    pid: Some(4242),
                    namespace: None,
                    pod_name: None,
                    container_name: None,
                },
                control_plane: ObservedControlPlane {
                    schema_version: 2,
                    omegon_version: "0.15.10-rc.17".into(),
                    base_url: format!("http://127.0.0.1/{instance_id}"),
                    startup_url: format!("http://127.0.0.1/{instance_id}/api/startup"),
                    health_url: format!("http://127.0.0.1/{instance_id}/api/healthz"),
                    ready_url: format!("http://127.0.0.1/{instance_id}/api/readyz"),
                    ws_url: format!("ws://127.0.0.1/{instance_id}/ws"),
                    auth_mode: "ephemeral-bearer".into(),
                    token_ref: Some("secret://auspex/instances/token".into()),
                    last_ready_at: Some("2026-04-03T12:00:11Z".into()),
                },
                health: ObservedHealth {
                    ready: true,
                    degraded_reason: None,
                    last_heartbeat_at: Some("2026-04-03T12:03:42Z".into()),
                    last_seen_at: Some("2026-04-03T12:03:42Z".into()),
                    freshness: Some(crate::runtime_types::InstanceFreshness::Fresh),
                },
                exit: ObservedExit {
                    exited: false,
                    exit_code: None,
                    exit_reason: None,
                    exited_at: None,
                },
            },
        }
    }

    #[test]
    fn registry_round_trips_persisted_instance_records() {
        let store = InstanceRegistryStore {
            schema_version: 1,
            instances: vec![sample_record("omg_01"), sample_record("omg_02")],
        };

        let path = unique_temp_path("round-trip");
        persist(&path, &store).unwrap();

        assert_eq!(load_or_default(&path), store);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn load_or_default_returns_empty_registry_for_missing_path() {
        let path = unique_temp_path("missing");
        let store = load_or_default(&path);

        assert_eq!(store, InstanceRegistryStore::default());
    }

    #[test]
    fn load_or_default_returns_empty_registry_for_empty_file() {
        let path = unique_temp_path("empty");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "").unwrap();

        assert_eq!(load_or_default(&path), InstanceRegistryStore::default());

        let _ = std::fs::remove_file(path);
    }

    fn unique_temp_path(label: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("auspex-instance-registry-{label}-{nanos}-{}.json", std::process::id()));
        path
    }
}
