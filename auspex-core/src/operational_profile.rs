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

    pub fn is_required_profile_satisfied_by(&self, profile: &str) -> bool {
        self.required_profile == profile
    }
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
        }
    }
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

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDiscoveryPolicy {
    #[default]
    ReadOnly,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossProjectStatePolicy {
    #[default]
    ExplicitGrantOnly,
    Forbidden,
    Allowed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestrator_profile_declares_fleet_policy() {
        let profile = OperationalProfile::auspex_orchestrator("0.2.0");

        assert_eq!(profile.scope, OperationalScope::Fleet);
        assert_eq!(profile.required_profile, "auspex-orchestrator");
        assert!(profile.capabilities.dispatch);
        assert!(profile.capabilities.host_actions);
        assert_eq!(profile.policy.unknown_host_actions, UnknownHostActionPolicy::Deny);
        assert!(profile.policy.dispatch_requires_compatible_instance);
    }
}
