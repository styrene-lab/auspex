#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::capability_registry::{CapabilityKey, InstanceCapabilitySnapshot};
use crate::compatibility::CompatibilityStatus;
use crate::fleet_projection::{FleetInstanceProjection, FleetRuntimeProjection};
use crate::operational_profile::OperationalProfile;

pub const GATEWAY_PROJECTION_SCHEMA_VERSION: u32 = 1;
pub const METHOD_FLEET_STATUS: &str = "auspex/fleet/status";
pub const METHOD_INSTANCES_LIST: &str = "auspex/instances/list";
pub const METHOD_CAPABILITIES_QUERY: &str = "auspex/capabilities/query";

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

impl GatewayProjectionMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FleetStatus => METHOD_FLEET_STATUS,
            Self::InstancesList => METHOD_INSTANCES_LIST,
            Self::CapabilitiesQuery => METHOD_CAPABILITIES_QUERY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityNamespace {
    Acp,
    Auspex,
    Omegon,
    Styrene,
    HostAction,
    Nex,
    NexSubstrate,
    Armory,
    Evidence,
    ProjectRules,
    TddSavepoint,
    Tool,
    Binary,
    Package,
    Extension,
    Runtime,
    Service,
    Unknown,
}

impl CapabilityNamespace {
    pub fn from_key(key: &CapabilityKey) -> Self {
        match key.kind {
            crate::capability_registry::CapabilityKind::HostAction => Self::HostAction,
            crate::capability_registry::CapabilityKind::Binary => Self::Binary,
            crate::capability_registry::CapabilityKind::Package => Self::Package,
            crate::capability_registry::CapabilityKind::Extension => Self::Extension,
            crate::capability_registry::CapabilityKind::Runtime => Self::Runtime,
            crate::capability_registry::CapabilityKind::Service => Self::Service,
            crate::capability_registry::CapabilityKind::Tool => {
                if key.name.starts_with("acp/") || key.name.starts_with("acp.") {
                    Self::Acp
                } else if key.name.starts_with("auspex/") {
                    Self::Auspex
                } else if key.name.starts_with("omegon/") {
                    Self::Omegon
                } else if key.name.starts_with("styrene/") {
                    Self::Styrene
                } else if key.name.starts_with("nex_substrate")
                    || key.name.starts_with("nex-substrate")
                    || key.name.starts_with("nex/substrate")
                {
                    Self::NexSubstrate
                } else if key.name.starts_with("nex") || key.name.starts_with("nex/") {
                    Self::Nex
                } else if key.name.starts_with("evidence.") || key.name.starts_with("evidence/") {
                    Self::Evidence
                } else if key.name.starts_with("project-rules.") || key.name.starts_with("project_rules.") {
                    Self::ProjectRules
                } else if key.name.starts_with("tdd_savepoint.")
                    || key.name.starts_with("tdd-savepoint.")
                    || key.name.starts_with("tdd.savepoint.")
                {
                    Self::TddSavepoint
                } else if key.name.starts_with("armory") || key.name.starts_with("armory/") {
                    Self::Armory
                } else {
                    Self::Tool
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayMethodDescriptor {
    pub name: String,
    pub method: GatewayProjectionMethod,
    pub read_only: bool,
}

pub fn projection_method_registry() -> Vec<GatewayMethodDescriptor> {
    vec![
        GatewayMethodDescriptor {
            name: METHOD_FLEET_STATUS.into(),
            method: GatewayProjectionMethod::FleetStatus,
            read_only: true,
        },
        GatewayMethodDescriptor {
            name: METHOD_INSTANCES_LIST.into(),
            method: GatewayProjectionMethod::InstancesList,
            read_only: true,
        },
        GatewayMethodDescriptor {
            name: METHOD_CAPABILITIES_QUERY.into(),
            method: GatewayProjectionMethod::CapabilitiesQuery,
            read_only: true,
        },
    ]
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
        let mut fallback_surfaces = Vec::new();
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
        let missing_profile_count = fleet
            .instances
            .iter()
            .filter(|instance| instance.operational_profile.is_none())
            .count();
        if missing_profile_count > 0 {
            reasons.push(format!(
                "{} instance(s) have no operational profile metadata",
                missing_profile_count
            ));
            unavailable_surfaces.push("profile-gated-first-party-methods".into());
        }
        let no_host_action_support = fleet
            .instances
            .iter()
            .filter(|instance| instance.ready)
            .filter(|instance| !instance_supports_host_actions(instance))
            .count();
        if no_host_action_support > 0 {
            reasons.push(format!(
                "{} ready instance(s) have no known HostAction support",
                no_host_action_support
            ));
            unavailable_surfaces.push("auspex/host-actions/*".into());
        }

        let missing_evidence_read_model = fleet
            .instances
            .iter()
            .filter(|instance| instance.ready)
            .filter(|instance| {
                !instance
                    .operational_profile
                    .as_ref()
                    .is_some_and(|profile| profile.capabilities.evidence_read_model)
            })
            .count();
        if missing_evidence_read_model > 0 {
            reasons.push(format!(
                "{} ready instance(s) have no evidence substrate read model",
                missing_evidence_read_model
            ));
            unavailable_surfaces.push("omegon/evidence/*".into());
        }

        let missing_project_rules = fleet
            .instances
            .iter()
            .filter(|instance| instance.ready)
            .filter(|instance| {
                !instance
                    .operational_profile
                    .as_ref()
                    .is_some_and(|profile| profile.capabilities.project_rules)
            })
            .count();
        if missing_project_rules > 0 {
            reasons.push(format!(
                "{} ready instance(s) have no project-rules read model",
                missing_project_rules
            ));
            fallback_surfaces.push("project-rules:not-evaluated".into());
        }

        let missing_nex_substrate = fleet
            .instances
            .iter()
            .filter(|instance| instance.ready)
            .filter(|instance| {
                !instance
                    .operational_profile
                    .as_ref()
                    .is_some_and(|profile| profile.capabilities.nex_substrate)
            })
            .count();
        if missing_nex_substrate > 0 {
            reasons.push(format!(
                "{} ready instance(s) have no Nex substrate report",
                missing_nex_substrate
            ));
            fallback_surfaces.push("nex-substrate:advisory-degraded".into());
        }

        if reasons.is_empty() {
            Self::default()
        } else {
            Self {
                mode: GatewayDegradationMode::Degraded,
                reasons,
                unavailable_surfaces,
                fallback_surfaces: {
                    let mut surfaces = vec![
                        "auspex/fleet/status".into(),
                        "auspex/instances/list".into(),
                    ];
                    surfaces.extend(fallback_surfaces);
                    surfaces
                },
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
        Self {
            schema_version: GATEWAY_PROJECTION_SCHEMA_VERSION,
            query,
            matches,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayCapabilityMatch {
    pub instance_id: String,
    pub namespace: CapabilityNamespace,
    pub ready: bool,
    pub compatibility: CompatibilityStatus,
    pub capabilities: InstanceCapabilitySnapshot,
}

fn instance_supports_host_actions(instance: &FleetInstanceProjection) -> bool {
    instance
        .operational_profile
        .as_ref()
        .is_some_and(|profile: &OperationalProfile| profile.capabilities.host_actions)
        || instance.capabilities.as_ref().is_some_and(|capabilities| {
            capabilities.evidence.iter().any(|evidence| {
                CapabilityNamespace::from_key(&evidence.key) == CapabilityNamespace::HostAction
                    && evidence.status == crate::capability_registry::CapabilityStatus::Present
            })
        })
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
        namespace: CapabilityNamespace::from_key(query),
        ready: instance.ready,
        compatibility,
        capabilities: capabilities.clone(),
    })
}

pub mod fixtures {
    pub fn demo_instance(
        id: &str,
        version: &str,
        ready: bool,
        with_profile: bool,
        capabilities: &[&str],
    ) -> crate::runtime_types::InstanceRecord {
        let control_plane = crate::runtime_types::ObservedControlPlane {
            schema_version: 2,
            omegon_version: version.into(),
            base_url: format!("http://127.0.0.1/{id}"),
            ..Default::default()
        };
        crate::runtime_types::InstanceRecord {
            schema_version: 1,
            identity: crate::runtime_types::WorkerIdentity {
                instance_id: id.into(),
                role: if id.contains("bot") {
                    crate::runtime_types::WorkerRole::DetachedService
                } else if id.contains("worker") {
                    crate::runtime_types::WorkerRole::SupervisedChild
                } else {
                    crate::runtime_types::WorkerRole::PrimaryDriver
                },
                profile: if id.contains("discord") {
                    "discord-bot".into()
                } else if id.contains("worker") {
                    "coding-agent-worker".into()
                } else {
                    "coding-agent-primary".into()
                },
                raw_role: None,
                raw_profile: None,
                raw_runtime_profile: None,
                status: crate::runtime_types::WorkerLifecycleState::Ready,
                created_at: "2026-05-30T00:00:00Z".into(),
                updated_at: "2026-05-30T00:00:00Z".into(),
            },
            observed: crate::runtime_types::ObservedWorkerState {
                control_plane: control_plane.clone(),
                health: crate::runtime_types::ObservedHealth { ready, ..Default::default() },
                compatibility: Some(crate::compatibility::assess_observed_control_plane(&control_plane)),
                operational_profile: with_profile.then(|| crate::operational_profile::OperationalProfile::auspex_orchestrator("0.2.0")),
                capabilities: Some(crate::capability_registry::InstanceCapabilitySnapshot::from_instance_descriptor_capabilities(
                    id.to_string(),
                    capabilities.iter().copied(),
                )),
                ..Default::default()
            },
            ..Default::default()
        }
    }
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
                raw_role: None,
                raw_profile: None,
                raw_runtime_profile: None,
                status: WorkerLifecycleState::Ready,
                created_at: "2026-05-30T00:00:00Z".into(),
                updated_at: "2026-05-30T00:00:00Z".into(),
            },
            desired: DesiredWorkerState::default(),
            observed: ObservedWorkerState {
                control_plane: control_plane.clone(),
                health: ObservedHealth {
                    ready,
                    ..Default::default()
                },
                compatibility: Some(assess_observed_control_plane(&control_plane)),
                operational_profile: Some(
                    crate::operational_profile::OperationalProfile::auspex_orchestrator("0.2.0"),
                ),
                capabilities: Some(
                    InstanceCapabilitySnapshot::from_instance_descriptor_capabilities(
                        id.to_string(),
                        capabilities.iter().copied(),
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn golden_empty_fleet_status_json_is_stable() {
        let response =
            GatewayProjectionResponse::fleet_status(FleetRuntimeProjection::from_instances(&[]));
        let actual = serde_json::to_value(&response).unwrap();
        let expected = serde_json::json!({
            "schema_version": 1,
            "method": "fleet-status",
            "degradation": {
                "mode": "degraded",
                "reasons": ["no fleet instances are registered"],
                "unavailable_surfaces": ["auspex/dispatch/submit"],
                "fallback_surfaces": ["auspex/fleet/status"]
            },
            "fleet": {
                "schema_version": 1,
                "instances": [],
                "host_action_queue": [],
                "summary": {
                    "total_instances": 0,
                    "ready_instances": 0,
                    "compatible_instances": 0,
                    "unsupported_instances": 0,
                    "pending_host_actions": 0
                }
            }
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn golden_instances_list_json_excludes_raw_registry_shape() {
        let response =
            GatewayInstancesListResponse::from_fleet(FleetRuntimeProjection::from_instances(&[
                instance("primary", "0.25.6", true, &["state.snapshot"]),
            ]));
        let actual = serde_json::to_value(&response).unwrap();

        assert_eq!(actual["schema_version"], 1);
        assert_eq!(actual["degradation"]["mode"], "full");
        assert_eq!(actual["instances"].as_array().unwrap().len(), 1);
        assert_eq!(actual["instances"][0]["instance_id"], "primary");
        assert!(actual["instances"][0].get("desired").is_none());
        assert!(actual["instances"][0].get("observed").is_none());
        assert!(actual["instances"][0].get("ownership").is_none());
    }

    #[test]
    fn golden_capability_query_json_is_stable_and_namespaced() {
        let fleet = FleetRuntimeProjection::from_instances(&[
            instance("primary", "0.25.6", true, &["omegon/context/status"]),
            instance("worker", "0.25.6", true, &["events.stream"]),
        ]);
        let response = GatewayCapabilitiesQueryResponse::from_fleet(
            &fleet,
            CapabilityKey::tool("omegon/context/status"),
        );
        let actual = serde_json::to_value(&response).unwrap();

        assert_eq!(actual["schema_version"], 1);
        assert_eq!(actual["query"]["kind"], "tool");
        assert_eq!(actual["query"]["name"], "omegon/context/status");
        assert_eq!(actual["matches"].as_array().unwrap().len(), 1);
        assert_eq!(actual["matches"][0]["instance_id"], "primary");
        assert_eq!(actual["matches"][0]["namespace"], "omegon");
        assert_eq!(actual["matches"][0]["compatibility"], "compatible");
    }

    #[test]
    fn no_compatible_instances_is_unsupported_not_degraded() {
        let fleet = FleetRuntimeProjection::from_instances(&[instance(
            "old",
            "0.23.0",
            false,
            &["state.snapshot"],
        )]);
        let response = GatewayProjectionResponse::fleet_status(fleet);

        assert_eq!(
            response.degradation.mode,
            GatewayDegradationMode::Unsupported
        );
        assert!(
            response
                .degradation
                .unavailable_surfaces
                .contains(&"auspex/host-actions/*".to_string())
        );
    }

    #[test]
    fn method_registry_is_read_only_and_canonical() {
        let methods = projection_method_registry();

        assert_eq!(methods.len(), 3);
        assert!(methods.iter().all(|method| method.read_only));
        assert!(
            methods
                .iter()
                .any(|method| method.name == METHOD_FLEET_STATUS)
        );
        assert_eq!(
            GatewayProjectionMethod::FleetStatus.as_str(),
            METHOD_FLEET_STATUS
        );
    }

    #[test]
    fn capability_namespace_detects_first_party_surfaces() {
        assert_eq!(
            CapabilityNamespace::from_key(&CapabilityKey::tool("omegon/context/status")),
            CapabilityNamespace::Omegon
        );
        assert_eq!(
            CapabilityNamespace::from_key(&CapabilityKey::tool("styrene/identity/attest")),
            CapabilityNamespace::Styrene
        );
        assert_eq!(
            CapabilityNamespace::from_key(&CapabilityKey::host_action("package.install@1")),
            CapabilityNamespace::HostAction
        );
    }

    #[test]
    fn missing_profile_and_hostaction_support_degrade_projection() {
        let mut record = instance("primary", "0.25.6", true, &["state.snapshot"]);
        record.observed.operational_profile = None;
        let fleet = FleetRuntimeProjection::from_instances(&[record]);
        let response = GatewayProjectionResponse::fleet_status(fleet);

        assert_eq!(response.degradation.mode, GatewayDegradationMode::Degraded);
        assert!(
            response
                .degradation
                .reasons
                .iter()
                .any(|reason| reason.contains("operational profile"))
        );
        assert!(
            response
                .degradation
                .unavailable_surfaces
                .contains(&"auspex/host-actions/*".to_string())
        );
    }

    #[test]
    fn fleet_status_serializes_stable_method_and_schema() {
        let fleet = FleetRuntimeProjection::from_instances(&[instance(
            "primary",
            "0.25.6",
            true,
            &["state.snapshot"],
        )]);
        let response = GatewayProjectionResponse::fleet_status(fleet);
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"method\":\"fleet-status\""));
        assert!(json.contains("\"mode\":\"full\""));
    }

    #[test]
    fn empty_fleet_projection_degrades_without_dispatch() {
        let response =
            GatewayProjectionResponse::fleet_status(FleetRuntimeProjection::from_instances(&[]));

        assert_eq!(response.degradation.mode, GatewayDegradationMode::Degraded);
        assert!(
            response
                .degradation
                .unavailable_surfaces
                .contains(&"auspex/dispatch/submit".to_string())
        );
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
        assert!(
            response
                .degradation
                .reasons
                .iter()
                .any(|reason| reason.contains("unsupported"))
        );
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

    #[test]
    fn capability_namespace_detects_omegon_026_evidence_surfaces() {
        assert_eq!(
            CapabilityNamespace::from_key(&CapabilityKey::tool("evidence.map.read")),
            CapabilityNamespace::Evidence
        );
        assert_eq!(
            CapabilityNamespace::from_key(&CapabilityKey::tool("project-rules.check")),
            CapabilityNamespace::ProjectRules
        );
        assert_eq!(
            CapabilityNamespace::from_key(&CapabilityKey::tool("nex_substrate.devenv.inspect")),
            CapabilityNamespace::NexSubstrate
        );
        assert_eq!(
            CapabilityNamespace::from_key(&CapabilityKey::tool("tdd_savepoint.evidence")),
            CapabilityNamespace::TddSavepoint
        );
    }

}
