#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationalProfile {
    pub name: String,
    pub version: String,
    pub scope: OperationalScope,
    pub recommended_profile: String,
    pub required_profile: String,
    pub capability_contract_version: u32,
    pub capabilities: OperationalCapabilities,
    pub policy: OperationalPolicy,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, serde_json::Value>,
}

impl OperationalProfile {
    pub fn auspex_orchestrator(version: impl Into<String>) -> Self {
        Self {
            name: "auspex-orchestrator".into(),
            version: version.into(),
            scope: OperationalScope::Fleet,
            recommended_profile: "auspex-orchestrator".into(),
            required_profile: "auspex-orchestrator".into(),
            capability_contract_version: 1,
            capabilities: OperationalCapabilities::orchestrator(),
            policy: OperationalPolicy::default_orchestrator(),
            meta: BTreeMap::new(),
        }
    }

    pub fn from_initialize_metadata(metadata: &serde_json::Value) -> Option<Self> {
        let root = metadata
            .pointer("/_meta/auspex")
            .or_else(|| metadata.pointer("/meta/auspex"))
            .unwrap_or(metadata);
        let info = root
            .get("runtime_info")
            .or_else(|| root.get("extension_info"))?;
        let capabilities = root.get("capabilities");
        let policy = root.get("policy");

        let name = string_field(info, "name")?;
        let version = string_field(info, "version").unwrap_or_default();
        let recommended_profile =
            string_field(info, "recommended_profile").unwrap_or_else(|| name.clone());
        let required_profile =
            string_field(info, "required_profile").unwrap_or_else(|| recommended_profile.clone());
        let capability_contract_version = info
            .get("capability_contract_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(1) as u32;
        let scope = string_field(info, "scope")
            .and_then(|value| OperationalScope::from_str(&value))
            .unwrap_or_default();

        Some(Self {
            name,
            version,
            scope,
            recommended_profile,
            required_profile,
            capability_contract_version,
            capabilities: OperationalCapabilities::from_metadata(capabilities),
            policy: OperationalPolicy::from_metadata(policy),
            meta: BTreeMap::new(),
        })
    }


    pub fn from_omegon_runtime_evidence(
        instance: &crate::omegon_control::OmegonInstanceDescriptor,
        harness: Option<&serde_json::Value>,
    ) -> Self {
        let runtime = instance.runtime.as_ref();
        let control_plane = instance.control_plane.as_ref();
        let runtime_profile = runtime
            .and_then(|runtime| runtime.runtime_profile.clone())
            .or_else(|| {
                harness
                    .and_then(|harness| harness.get("runtime_profile"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "unknown".into());
        let version = control_plane
            .and_then(|control_plane| control_plane.omegon_version.clone())
            .unwrap_or_default();
        let capabilities = control_plane
            .map(|control_plane| control_plane.capabilities.as_slice())
            .unwrap_or(&[]);
        let has_capability = |needle: &str| capabilities.iter().any(|capability| capability == needle);
        let has_host_action = capabilities.iter().any(|capability| {
            capability.contains('@') || capability.starts_with("host_action.") || capability.starts_with("host-action.")
        });

        let mut meta = BTreeMap::new();
        meta.insert("source".into(), serde_json::Value::String("derived_from_omegon_state".into()));
        meta.insert("raw_runtime_profile".into(), serde_json::Value::String(runtime_profile.clone()));
        if let Some(runtime) = runtime {
            if let Some(value) = runtime.autonomy_mode.as_ref() {
                meta.insert("raw_autonomy_mode".into(), serde_json::Value::String(value.clone()));
            }
            if let Some(value) = runtime.context_class.as_ref() {
                meta.insert("raw_context_class".into(), serde_json::Value::String(value.clone()));
            }
            if let Some(value) = runtime.capability_tier.as_ref() {
                meta.insert("raw_capability_tier".into(), serde_json::Value::String(value.clone()));
            }
        }
        if let Some(harness) = harness {
            for key in ["operating_profile", "authorization", "principal_id", "identity_issuer", "posture", "session_kind"] {
                if let Some(value) = harness.get(key).and_then(|value| value.as_str()) {
                    meta.insert(format!("harness_{key}"), serde_json::Value::String(value.into()));
                }
            }
        }

        Self {
            name: "omegon-runtime-derived".into(),
            version,
            scope: OperationalScope::Host,
            recommended_profile: runtime_profile.clone(),
            required_profile: runtime_profile,
            capability_contract_version: 1,
            capabilities: OperationalCapabilities {
                instance_registry: has_capability("state.snapshot") || has_capability("session.list"),
                dispatch: has_capability("prompt.submit"),
                supervision: has_capability("dispatcher.switch") || has_capability("turn.cancel"),
                host_actions: has_host_action,
                package_reconciliation: has_capability("package.install@1"),
                audit: false,
                fleet_projection: has_capability("state.snapshot"),
                evidence_read_model: capabilities.iter().any(|capability| {
                    capability.starts_with("evidence.")
                        || capability.starts_with("evidence/")
                        || capability == "evidence.map.read"
                }),
                project_rules: capabilities.iter().any(|capability| {
                    capability.starts_with("project-rules.")
                        || capability.starts_with("project_rules.")
                        || capability == "project-rules.check"
                }),
                nex_substrate: capabilities.iter().any(|capability| {
                    capability.starts_with("nex_substrate.")
                        || capability.starts_with("nex-substrate.")
                        || capability.starts_with("nex/substrate")
                }),
                tdd_savepoint: capabilities.iter().any(|capability| {
                    capability.starts_with("tdd_savepoint.")
                        || capability.starts_with("tdd-savepoint.")
                        || capability.starts_with("tdd.savepoint.")
                }),
            },
            policy: OperationalPolicy::default_orchestrator(),
            meta,
        }
    }

    pub fn is_required_profile_satisfied_by(&self, profile: &str) -> bool {
        self.required_profile == profile
    }
}

fn string_field(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationalScope {
    Project,
    #[default]
    Fleet,
    Host,
    Cluster,
}

impl OperationalScope {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "project" => Some(Self::Project),
            "fleet" => Some(Self::Fleet),
            "host" => Some(Self::Host),
            "cluster" => Some(Self::Cluster),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationalCapabilities {
    #[serde(default)]
    pub instance_registry: bool,
    #[serde(default)]
    pub dispatch: bool,
    #[serde(default)]
    pub supervision: bool,
    #[serde(default)]
    pub host_actions: bool,
    #[serde(default)]
    pub package_reconciliation: bool,
    #[serde(default)]
    pub audit: bool,
    #[serde(default)]
    pub fleet_projection: bool,
    #[serde(default)]
    pub evidence_read_model: bool,
    #[serde(default)]
    pub project_rules: bool,
    #[serde(default)]
    pub nex_substrate: bool,
    #[serde(default)]
    pub tdd_savepoint: bool,
}

impl OperationalCapabilities {
    pub fn orchestrator() -> Self {
        Self {
            instance_registry: true,
            dispatch: true,
            supervision: true,
            host_actions: true,
            package_reconciliation: true,
            audit: true,
            fleet_projection: true,
            evidence_read_model: true,
            project_rules: true,
            nex_substrate: true,
            tdd_savepoint: true,
        }
    }

    fn from_metadata(value: Option<&serde_json::Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };
        Self {
            instance_registry: bool_field(value, "instance_registry"),
            dispatch: bool_field(value, "dispatch"),
            supervision: bool_field(value, "supervision"),
            host_actions: bool_field(value, "host_actions"),
            package_reconciliation: bool_field(value, "package_reconciliation"),
            audit: bool_field(value, "audit"),
            fleet_projection: bool_field(value, "fleet_projection"),
            evidence_read_model: bool_field(value, "evidence_read_model"),
            project_rules: bool_field(value, "project_rules"),
            nex_substrate: bool_field(value, "nex_substrate"),
            tdd_savepoint: bool_field(value, "tdd_savepoint"),
        }
    }
}

fn bool_field(value: &serde_json::Value, field: &str) -> bool {
    value
        .get(field)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationalPolicy {
    pub host_action_mutation_requires_approval: bool,
    pub unknown_host_actions: UnknownHostActionPolicy,
    pub capability_discovery: CapabilityDiscoveryPolicy,
    pub dispatch_requires_compatible_instance: bool,
    pub cross_project_state: CrossProjectStatePolicy,
}

impl OperationalPolicy {
    pub fn default_orchestrator() -> Self {
        Self {
            host_action_mutation_requires_approval: true,
            unknown_host_actions: UnknownHostActionPolicy::Deny,
            capability_discovery: CapabilityDiscoveryPolicy::ReadOnly,
            dispatch_requires_compatible_instance: true,
            cross_project_state: CrossProjectStatePolicy::ExplicitGrantOnly,
        }
    }

    fn from_metadata(value: Option<&serde_json::Value>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };
        Self {
            host_action_mutation_requires_approval: bool_field(
                value,
                "host_action_mutation_requires_approval",
            ),
            unknown_host_actions: value
                .get("unknown_host_actions")
                .and_then(|value| value.as_str())
                .and_then(UnknownHostActionPolicy::from_str)
                .unwrap_or_default(),
            capability_discovery: value
                .get("capability_discovery")
                .and_then(|value| value.as_str())
                .and_then(CapabilityDiscoveryPolicy::from_str)
                .unwrap_or_default(),
            dispatch_requires_compatible_instance: bool_field(
                value,
                "dispatch_requires_compatible_instance",
            ),
            cross_project_state: value
                .get("cross_project_state")
                .and_then(|value| value.as_str())
                .and_then(CrossProjectStatePolicy::from_str)
                .unwrap_or_default(),
        }
    }
}

impl Default for OperationalPolicy {
    fn default() -> Self {
        Self::default_orchestrator()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownHostActionPolicy {
    #[default]
    Deny,
    Unsupported,
}

impl UnknownHostActionPolicy {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "deny" => Some(Self::Deny),
            "unsupported" => Some(Self::Unsupported),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDiscoveryPolicy {
    #[default]
    ReadOnly,
}

impl CapabilityDiscoveryPolicy {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "read_only" => Some(Self::ReadOnly),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProjectStatePolicy {
    #[default]
    ExplicitGrantOnly,
    Forbidden,
    Allowed,
}

impl CrossProjectStatePolicy {
    fn from_str(value: &str) -> Option<Self> {
        match value {
            "explicit_grant_only" => Some(Self::ExplicitGrantOnly),
            "forbidden" => Some(Self::Forbidden),
            "allowed" => Some(Self::Allowed),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn orchestrator_profile_declares_fleet_policy() {
        let profile = OperationalProfile::auspex_orchestrator("0.2.0");

        assert_eq!(profile.scope, OperationalScope::Fleet);
        assert_eq!(profile.required_profile, "auspex-orchestrator");
        assert!(profile.capabilities.dispatch);
        assert!(profile.capabilities.host_actions);
        assert_eq!(
            profile.policy.unknown_host_actions,
            UnknownHostActionPolicy::Deny
        );
        assert!(profile.policy.dispatch_requires_compatible_instance);
    }

    #[test]
    fn parses_auspex_meta_initialize_metadata() {
        let metadata = json!({
            "_meta": {
                "auspex": {
                    "runtime_info": {
                        "name": "auspex-orchestrator",
                        "version": "0.2.0",
                        "scope": "fleet",
                        "recommended_profile": "auspex-orchestrator",
                        "required_profile": "auspex-orchestrator",
                        "capability_contract_version": 3
                    },
                    "capabilities": {
                        "instance_registry": true,
                        "dispatch": true,
                        "host_actions": true,
                        "fleet_projection": true
                    },
                    "policy": {
                        "host_action_mutation_requires_approval": true,
                        "unknown_host_actions": "deny",
                        "capability_discovery": "read_only",
                        "dispatch_requires_compatible_instance": true,
                        "cross_project_state": "explicit_grant_only"
                    }
                }
            }
        });

        let profile = OperationalProfile::from_initialize_metadata(&metadata).unwrap();

        assert_eq!(profile.name, "auspex-orchestrator");
        assert_eq!(profile.capability_contract_version, 3);
        assert!(profile.capabilities.instance_registry);
        assert!(profile.capabilities.dispatch);
        assert!(profile.policy.host_action_mutation_requires_approval);
    }

    #[test]
    fn parses_flynt_style_extension_initialize_metadata() {
        let metadata = json!({
            "extension_info": {
                "name": "flynt",
                "version": "0.1.0",
                "scope": "project",
                "recommended_profile": "flynt-agent",
                "required_profile": "flynt-agent",
                "capability_contract_version": 1
            },
            "capabilities": {
                "host_actions": false,
                "audit": true
            },
            "policy": {
                "unknown_host_actions": "deny",
                "capability_discovery": "read_only",
                "cross_project_state": "forbidden"
            }
        });

        let profile = OperationalProfile::from_initialize_metadata(&metadata).unwrap();

        assert_eq!(profile.name, "flynt");
        assert_eq!(profile.scope, OperationalScope::Project);
        assert_eq!(profile.required_profile, "flynt-agent");
        assert!(profile.capabilities.audit);
        assert_eq!(
            profile.policy.cross_project_state,
            CrossProjectStatePolicy::Forbidden
        );
    }

    #[test]
    fn derives_profile_from_omegon_runtime_evidence() {
        let descriptor = crate::omegon_control::OmegonInstanceDescriptor {
            identity: crate::omegon_control::OmegonInstanceIdentity {
                instance_id: "web-compat".into(),
                role: "primary_driver".into(),
                profile: "long-running-daemon".into(),
                status: "ready".into(),
            },
            control_plane: Some(crate::omegon_control::OmegonControlPlaneDescriptor {
                omegon_version: Some("0.25.4".into()),
                capabilities: vec![
                    "state.snapshot".into(),
                    "prompt.submit".into(),
                    "shutdown".into(),
                    "evidence.map.read".into(),
                    "project-rules.check".into(),
                    "nex_substrate.devenv.inspect".into(),
                    "tdd_savepoint.evidence".into(),
                ],
                ..Default::default()
            }),
            runtime: Some(crate::omegon_control::OmegonRuntimeDescriptor {
                runtime_profile: Some("primary_interactive".into()),
                autonomy_mode: Some("operator_driven".into()),
                capability_tier: Some("victory".into()),
                context_class: Some("Squad".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let harness = serde_json::json!({
            "operating_profile": "anonymous / Architect / Medium / Clan",
            "principal_id": "local-operator"
        });

        let profile = OperationalProfile::from_omegon_runtime_evidence(&descriptor, Some(&harness));

        assert_eq!(profile.name, "omegon-runtime-derived");
        assert_eq!(profile.required_profile, "primary_interactive");
        assert!(profile.capabilities.instance_registry);
        assert!(profile.capabilities.dispatch);
        assert!(profile.capabilities.evidence_read_model);
        assert!(profile.capabilities.project_rules);
        assert!(profile.capabilities.nex_substrate);
        assert!(profile.capabilities.tdd_savepoint);
        assert!(!profile.capabilities.host_actions);
        assert_eq!(profile.meta.get("source").and_then(|v| v.as_str()), Some("derived_from_omegon_state"));
        assert_eq!(profile.meta.get("harness_principal_id").and_then(|v| v.as_str()), Some("local-operator"));
    }

}
