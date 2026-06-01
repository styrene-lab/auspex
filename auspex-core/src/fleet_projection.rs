#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::capability_registry::InstanceCapabilitySnapshot;
use crate::compatibility::{CompatibilityAssessment, CompatibilityStatus};
use crate::host_action_policy::HostActionPolicyDecision;
use crate::operational_profile::OperationalProfile;
use crate::runtime_types::{InstanceRecord, WorkerLifecycleState};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetRuntimeProjection {
    pub schema_version: u32,
    #[serde(default)]
    pub instances: Vec<FleetInstanceProjection>,
    #[serde(default)]
    pub host_action_queue: Vec<HostActionQueueProjection>,
    pub summary: FleetRuntimeSummary,
}

impl FleetRuntimeProjection {
    pub fn from_instances(instances: &[InstanceRecord]) -> Self {
        let projected: Vec<FleetInstanceProjection> = instances
            .iter()
            .map(FleetInstanceProjection::from_instance_record)
            .collect();
        let summary = FleetRuntimeSummary::from_instances(&projected);
        Self {
            schema_version: 1,
            instances: projected,
            host_action_queue: Vec::new(),
            summary,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetRuntimeSummary {
    pub total_instances: usize,
    pub ready_instances: usize,
    pub compatible_instances: usize,
    pub unsupported_instances: usize,
    pub pending_host_actions: usize,
}

impl FleetRuntimeSummary {
    fn from_instances(instances: &[FleetInstanceProjection]) -> Self {
        Self {
            total_instances: instances.len(),
            ready_instances: instances.iter().filter(|i| i.ready).count(),
            compatible_instances: instances
                .iter()
                .filter(|i| {
                    i.compatibility
                        .as_ref()
                        .is_some_and(|c| c.status == CompatibilityStatus::Compatible)
                })
                .count(),
            unsupported_instances: instances
                .iter()
                .filter(|i| {
                    i.compatibility
                        .as_ref()
                        .is_some_and(|c| c.status == CompatibilityStatus::Unsupported)
                })
                .count(),
            pending_host_actions: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FleetInstanceProjection {
    pub instance_id: String,
    pub role: String,
    pub profile: String,
    pub lifecycle: WorkerLifecycleState,
    pub ready: bool,
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility: Option<CompatibilityAssessment>,
    pub compatibility_status: CompatibilityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operational_profile: Option<OperationalProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<InstanceCapabilitySnapshot>,
}

impl FleetInstanceProjection {
    pub fn from_instance_record(record: &InstanceRecord) -> Self {
        Self {
            instance_id: record.identity.instance_id.clone(),
            role: record.identity.role.label().into(),
            profile: record.identity.profile.clone(),
            lifecycle: record.identity.status,
            ready: record.observed.health.ready,
            base_url: record.observed.control_plane.base_url.clone(),
            acp_url: record.observed.control_plane.acp_url.clone(),
            compatibility_status: record
                .observed
                .compatibility
                .as_ref()
                .map(|compatibility| compatibility.status.clone())
                .unwrap_or(CompatibilityStatus::Unknown),
            compatibility: record.observed.compatibility.clone(),
            operational_profile: record.observed.operational_profile.clone(),
            capabilities: record.observed.capabilities.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostActionQueueProjection {
    pub request_id: String,
    pub instance_id: String,
    pub action_type: String,
    pub decision: HostActionPolicyDecision,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compatibility::assess_observed_control_plane;
    use crate::runtime_types::{
        BackendConfig, BackendKind, DesiredWorkerState, ObservedControlPlane, ObservedExit,
        ObservedHealth, ObservedPlacement, ObservedWorkerState, OwnerKind, WorkerIdentity,
        WorkerOwnership, WorkerRole, WorkspaceBinding,
    };

    fn record(id: &str, version: &str, ready: bool) -> InstanceRecord {
        let control_plane = ObservedControlPlane {
            schema_version: 2,
            omegon_version: version.into(),
            base_url: format!("http://127.0.0.1/{id}"),
            startup_url: format!("http://127.0.0.1/{id}/api/startup"),
            health_url: format!("http://127.0.0.1/{id}/api/healthz"),
            ready_url: format!("http://127.0.0.1/{id}/api/readyz"),
            ws_url: format!("ws://127.0.0.1/{id}/ws"),
            acp_url: Some(format!("ws://127.0.0.1/{id}/acp")),
            auth_mode: "ephemeral-bearer".into(),
            ..Default::default()
        };
        let compatibility = assess_observed_control_plane(&control_plane);
        InstanceRecord {
            schema_version: 1,
            identity: WorkerIdentity {
                instance_id: id.into(),
                role: WorkerRole::PrimaryDriver,
                profile: "auspex-orchestrator".into(),
                status: WorkerLifecycleState::Ready,
                created_at: "2026-05-30T00:00:00Z".into(),
                updated_at: "2026-05-30T00:00:00Z".into(),
            },
            ownership: WorkerOwnership {
                owner_kind: OwnerKind::AuspexSession,
                owner_id: "operator".into(),
                parent_instance_id: None,
            },
            desired: DesiredWorkerState {
                backend: BackendConfig {
                    kind: BackendKind::LocalProcess,
                    image: None,
                    namespace: None,
                    resources: None,
                },
                workspace: WorkspaceBinding {
                    cwd: "/repo".into(),
                    workspace_id: "repo:test".into(),
                    branch: None,
                },
                task: None,
                policy: Default::default(),
                security: Default::default(),
            },
            observed: ObservedWorkerState {
                placement: ObservedPlacement {
                    placement_id: format!("pid:{id}"),
                    host: "localhost".into(),
                    pid: Some(42),
                    namespace: None,
                    pod_name: None,
                    container_name: None,
                },
                control_plane,
                health: ObservedHealth {
                    ready,
                    degraded_reason: None,
                    last_heartbeat_at: None,
                    last_seen_at: None,
                    freshness: None,
                },
                exit: ObservedExit::default(),
                compatibility: Some(compatibility),
                operational_profile: Some(OperationalProfile::auspex_orchestrator("0.2.0")),
                capabilities: None,
            },
        }
    }

    #[test]
    fn projection_summarizes_compatibility_and_readiness() {
        let projection = FleetRuntimeProjection::from_instances(&[
            record("ok", "0.25.4", true),
            record("old", "0.23.0", false),
        ]);

        assert_eq!(projection.summary.total_instances, 2);
        assert_eq!(projection.summary.ready_instances, 1);
        assert_eq!(projection.summary.compatible_instances, 1);
        assert_eq!(projection.summary.unsupported_instances, 1);
    }
}
