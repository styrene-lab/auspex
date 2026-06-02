#![allow(dead_code)]

use crate::capability_registry::InstanceCapabilitySnapshot;
use crate::compatibility::assess_observed_control_plane;
use crate::omegon_control::OmegonInstanceDescriptor;
use crate::runtime_types::{ObservedControlPlane, ObservedWorkerState};

pub fn apply_descriptor_to_observed_state(
    instance_id: &str,
    observed: &mut ObservedWorkerState,
    descriptor: &OmegonInstanceDescriptor,
) {
    apply_descriptor_and_metadata_to_observed_state(instance_id, observed, descriptor, None);
}

pub fn apply_descriptor_and_metadata_to_observed_state(
    instance_id: &str,
    observed: &mut ObservedWorkerState,
    descriptor: &OmegonInstanceDescriptor,
    metadata: Option<&serde_json::Value>,
) {
    if let Some(control_plane) = descriptor.control_plane.as_ref() {
        apply_control_plane_descriptor(&mut observed.control_plane, control_plane);
        observed.compatibility = Some(assess_observed_control_plane(&observed.control_plane));
        observed.capabilities = Some(
            InstanceCapabilitySnapshot::from_instance_descriptor_capabilities(
                instance_id.to_string(),
                control_plane.capabilities.clone(),
            ),
        );
    }
    if let Some(metadata) = metadata {
        observed.operational_profile =
            crate::operational_profile::OperationalProfile::from_initialize_metadata(metadata);
    }
}

pub fn apply_fixture_descriptor_and_metadata_to_observed_state(
    instance_id: &str,
    observed: &mut ObservedWorkerState,
    descriptor: &crate::fixtures::InstanceDescriptorData,
    metadata: Option<&serde_json::Value>,
) {
    if let Some(control_plane) = descriptor.control_plane.as_ref() {
        observed.control_plane.schema_version = control_plane.schema_version;
        if let Some(version) = control_plane
            .omegon_version
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            observed.control_plane.omegon_version = version.clone();
        }
        if let Some(base_url) = control_plane
            .base_url
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            observed.control_plane.base_url = base_url.clone();
        }
        observed.compatibility = Some(assess_observed_control_plane(&observed.control_plane));
        observed.capabilities = Some(
            InstanceCapabilitySnapshot::from_instance_descriptor_capabilities(
                instance_id.to_string(),
                control_plane.capabilities.clone(),
            ),
        );
    }
    if let Some(metadata) = metadata {
        observed.operational_profile =
            crate::operational_profile::OperationalProfile::from_initialize_metadata(metadata);
    }
}

fn apply_control_plane_descriptor(
    observed: &mut ObservedControlPlane,
    descriptor: &crate::omegon_control::OmegonControlPlaneDescriptor,
) {
    if descriptor.schema_version != 0 {
        observed.schema_version = descriptor.schema_version;
    }
    if let Some(value) = descriptor
        .omegon_version
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        observed.omegon_version = value.clone();
    }
    if let Some(value) = descriptor
        .base_url
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        observed.base_url = value.clone();
    }
    if let Some(value) = descriptor
        .startup_url
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        observed.startup_url = value.clone();
    }
    if let Some(value) = descriptor
        .health_url
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        observed.health_url = value.clone();
    }
    if let Some(value) = descriptor
        .ready_url
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        observed.ready_url = value.clone();
    }
    if let Some(value) = descriptor.ws_url.as_ref().filter(|value| !value.is_empty()) {
        observed.ws_url = value.clone();
    }
    if descriptor.acp_url.is_some() {
        observed.acp_url = descriptor.acp_url.clone();
    }
    if let Some(value) = descriptor
        .auth_mode
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        observed.auth_mode = value.clone();
    }
    if descriptor.token_ref.is_some() {
        observed.token_ref = descriptor.token_ref.clone();
    }
    if descriptor.transport_security.is_some() {
        observed.transport_security = descriptor.transport_security.clone();
    }
    if descriptor.mtls.is_some() {
        observed.mtls = descriptor.mtls;
    }
    if descriptor.last_ready_at.is_some() {
        observed.last_ready_at = descriptor.last_ready_at.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability_registry::{CapabilityKey, CapabilityStatus};
    use crate::compatibility::CompatibilityStatus;
    use crate::omegon_control::{OmegonControlPlaneDescriptor, OmegonInstanceDescriptor};

    #[test]
    fn descriptor_updates_compatibility_and_capabilities() {
        let descriptor = OmegonInstanceDescriptor {
            control_plane: Some(OmegonControlPlaneDescriptor {
                schema_version: 2,
                omegon_version: Some("0.25.4".into()),
                base_url: Some("http://127.0.0.1:7842".into()),
                capabilities: vec!["state.snapshot".into(), "events.stream".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut observed = ObservedWorkerState::default();

        apply_descriptor_to_observed_state("omg-1", &mut observed, &descriptor);

        assert_eq!(observed.control_plane.omegon_version, "0.25.4");
        assert_eq!(
            observed
                .compatibility
                .as_ref()
                .map(|assessment| &assessment.status),
            Some(&CompatibilityStatus::Compatible)
        );
        let capabilities = observed.capabilities.as_ref().unwrap();
        let state_snapshot = capabilities
            .evidence
            .iter()
            .find(|evidence| evidence.key == CapabilityKey::tool("state.snapshot"))
            .expect("state.snapshot evidence");
        assert_eq!(state_snapshot.status, CapabilityStatus::Present);
    }

    #[test]
    fn descriptor_with_metadata_updates_operational_profile() {
        let descriptor = OmegonInstanceDescriptor {
            control_plane: Some(OmegonControlPlaneDescriptor {
                schema_version: 2,
                omegon_version: Some("0.25.4".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let metadata = serde_json::json!({
            "_meta": {
                "auspex": {
                    "runtime_info": {
                        "name": "auspex-orchestrator",
                        "version": "0.2.0",
                        "scope": "fleet",
                        "required_profile": "auspex-orchestrator"
                    },
                    "capabilities": { "dispatch": true },
                    "policy": { "unknown_host_actions": "deny" }
                }
            }
        });
        let mut observed = ObservedWorkerState::default();

        apply_descriptor_and_metadata_to_observed_state(
            "omg-1",
            &mut observed,
            &descriptor,
            Some(&metadata),
        );

        let profile = observed.operational_profile.as_ref().unwrap();
        assert_eq!(profile.name, "auspex-orchestrator");
        assert!(profile.capabilities.dispatch);
    }
}
