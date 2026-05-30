#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::capability_registry::{CapabilityKey, InstanceCapabilitySnapshot};
use crate::compatibility::CompatibilityStatus;
use crate::fleet_projection::{FleetInstanceProjection, FleetRuntimeProjection};

pub const GATEWAY_PROJECTION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayProjectionResponse {
    pub schema_version: u32,
    pub method: GatewayProjectionMethod,
    pub degradation: GatewayDegradation,
    pub fleet: FleetRuntimeProjection,
}

impl GatewayProjectionResponse {
    pub fn fleet_status(fleet: FleetRuntimeProjection) -> Self {
        let degradation = GatewayDegradation::from_fleet(&fleet);
        Self {
            schema_version: GATEWAY_PROJECTION_SCHEMA_VERSION,
            method: GatewayProjectionMethod::FleetStatus,
            degradation,
            fleet,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GatewayProjectionMethod {
    FleetStatus,
    InstancesList,
    CapabilitiesQuery,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GatewayDegradationMode {
    #[default]
    Full,
    Degraded,
    AcpOnly,
    Unsupported,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayDegradation {
    pub mode: GatewayDegradationMode,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub unavailable_surfaces: Vec<String>,
    #[serde(default)]
    pub fallback_surfaces: Vec<String>,
}

impl GatewayDegradation {
    pub fn from_fleet(fleet: &FleetRuntimeProjection) -> Self {
        if fleet.summary.total_instances == 0 {
            return Self {
                mode: GatewayDegradationMode::Degraded,
                reasons: vec!["no fleet instances are registered".into()],
                unavailable_surfaces: vec!["auspex/dispatch/submit".into()],
                fallback_surfaces: vec!["auspex/fleet/status".into()],
            };
        }

        if fleet.summary.compatible_instances == 0 {
            return Self {
                mode: GatewayDegradationMode::Unsupported,
                reasons: vec!["no compatible Omegon instances are available".into()],
                unavailable_surfaces: vec![
                    "auspex/dispatch/submit".into(),
                    "auspex/host-actions/*".into(),
                ],
                fallback_surfaces: vec!["auspex/instances/list".into()],
            };
        }

        let mut reasons = Vec::new();
        let mut unavailable_surfaces = Vec::new();
        if fleet.summary.unsupported_instances > 0 {
            reasons.push(format!(
                "{} unsupported instance(s) present",
                fleet.summary.unsupported_instances
            ));
            unavailable_surfaces.push("dispatch-to-unsupported-instances".into());
        }
        if fleet.summary.ready_instances < fleet.summary.total_instances {
            reasons.push(format!(
                "{} instance(s) not ready",
                fleet.summary.total_instances - fleet.summary.ready_instances
            ));
            unavailable_surfaces.push("dispatch-to-not-ready-instances".into());
        }

        if reasons.is_empty() {
            Self::default()
        } else {
            Self {
                mode: GatewayDegradationMode::Degraded,
                reasons,
                unavailable_surfaces,
                fallback_surfaces: vec!["auspex/fleet/status".into(), "auspex/instances/list".into()],
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayInstancesListResponse {
    pub schema_version: u32,
    pub degradation: GatewayDegradation,
    pub instances: Vec<FleetInstanceProjection>,
}

impl GatewayInstancesListResponse {
    pub fn from_fleet(fleet: FleetRuntimeProjection) -> Self {
        Self {
            schema_version: GATEWAY_PROJECTION_SCHEMA_VERSION,
            degradation: GatewayDegradation::from_fleet(&fleet),
            instances: fleet.instances,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayCapabilitiesQueryResponse {
    pub schema_version: u32,
    pub query: CapabilityKey,
    pub matches: Vec<GatewayCapabilityMatch>,
}

impl GatewayCapabilitiesQueryResponse {
    pub fn from_fleet(fleet: &FleetRuntimeProjection, query: CapabilityKey) -> Self {
        let matches = fleet
            .instances
            .iter()
            .filter_map(|instance| capability_match(instance, &query))
            .collect();
        Self { schema_version: GATEWAY_PROJECTION_SCHEMA_VERSION, query, matches }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayCapabilityMatch {
    pub instance_id: String,
    pub ready: bool,
    pub compatibility: CompatibilityStatus,
    pub capabilities: InstanceCapabilitySnapshot,
}

fn capability_match(
    instance: &FleetInstanceProjection,
    query: &CapabilityKey,
) -> Option<GatewayCapabilityMatch> {
    let capabilities = instance.capabilities.as_ref()?;
    if !capabilities.has_present(query) {
        return None;
    }
    let compatibility = instance
        .compatibility
        .as_ref()
        .map(|assessment| assessment.status.clone())
        .unwrap_or(CompatibilityStatus::Unknown);
    Some(GatewayCapabilityMatch {
        instance_id: instance.instance_id.clone(),
        ready: instance.ready,
        compatibility,
        capabilities: capabilities.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability_registry::InstanceCapabilitySnapshot;
    use crate::compatibility::assess_observed_control_plane;
    use crate::fleet_projection::FleetRuntimeProjection;
    use crate::runtime_types::{
        DesiredWorkerState, InstanceRecord, ObservedControlPlane, ObservedHealth,
        ObservedWorkerState, WorkerIdentity, WorkerLifecycleState, WorkerRole,
    };

    fn instance(id: &str, version: &str, ready: bool, capabilities: &[&str]) -> InstanceRecord {
        let control_plane = ObservedControlPlane {
            schema_version: 2,
            omegon_version: version.into(),
            base_url: format!("http://127.0.0.1/{id}"),
            ..Default::default()
        };
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
            desired: DesiredWorkerState::default(),
            observed: ObservedWorkerState {
                control_plane: control_plane.clone(),
                health: ObservedHealth { ready, ..Default::default() },
                compatibility: Some(assess_observed_control_plane(&control_plane)),
                capabilities: Some(InstanceCapabilitySnapshot::from_instance_descriptor_capabilities(
                    id.to_string(),
                    capabilities.iter().copied(),
                )),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn empty_fleet_projection_degrades_without_dispatch() {
        let response = GatewayProjectionResponse::fleet_status(FleetRuntimeProjection::from_instances(&[]));

        assert_eq!(response.degradation.mode, GatewayDegradationMode::Degraded);
        assert!(response
            .degradation
            .unavailable_surfaces
            .contains(&"auspex/dispatch/submit".to_string()));
    }

    #[test]
    fn compatible_ready_fleet_has_full_projection() {
        let fleet = FleetRuntimeProjection::from_instances(&[instance(
            "primary",
            "0.25.6",
            true,
            &["state.snapshot"],
        )]);
        let response = GatewayProjectionResponse::fleet_status(fleet);

        assert_eq!(response.degradation.mode, GatewayDegradationMode::Full);
        assert_eq!(response.fleet.summary.compatible_instances, 1);
    }

    #[test]
    fn unsupported_instance_degrades_projection() {
        let fleet = FleetRuntimeProjection::from_instances(&[
            instance("primary", "0.25.6", true, &["state.snapshot"]),
            instance("old", "0.23.0", false, &[]),
        ]);
        let response = GatewayProjectionResponse::fleet_status(fleet);

        assert_eq!(response.degradation.mode, GatewayDegradationMode::Degraded);
        assert!(response
            .degradation
            .reasons
            .iter()
            .any(|reason| reason.contains("unsupported")));
    }

    #[test]
    fn capabilities_query_returns_matching_instances_only() {
        let fleet = FleetRuntimeProjection::from_instances(&[
            instance("primary", "0.25.6", true, &["state.snapshot"]),
            instance("worker", "0.25.6", true, &["events.stream"]),
        ]);
        let response = GatewayCapabilitiesQueryResponse::from_fleet(
            &fleet,
            CapabilityKey::tool("state.snapshot"),
        );

        assert_eq!(response.matches.len(), 1);
        assert_eq!(response.matches[0].instance_id, "primary");
    }
}
