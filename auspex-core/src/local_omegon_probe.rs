use crate::authorization::{authorize_local_omegon_action, runtime_resource, LocalOmegonAction};
use crate::local_omegon_discovery::LocalOmegonCandidate;
use crate::omegon_control::OmegonStartupInfo;
use styrene_policy::{PolicyDecision, PrincipalRef};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalOmegonProbeStatus {
    AttachedReadOnly,
    PolicyDenied,
    MissingStartupUrl,
    StartupFetchFailed,
    StartupParseFailed,
    StateFetchFailed,
    StateParseFailed,
}

#[derive(Clone, Debug)]
pub struct LocalOmegonProbeResult {
    pub status: LocalOmegonProbeStatus,
    pub policy: PolicyDecision,
    pub startup_url: Option<String>,
    pub state_url: Option<String>,
    pub instance_id: Option<String>,
    pub omegon_version: Option<String>,
    pub capabilities: Vec<String>,
    pub evidence: String,
    pub controller: Option<crate::controller::AppController>,
}

impl LocalOmegonProbeResult {
    fn denied(policy: PolicyDecision, startup_url: Option<String>) -> Self {
        Self {
            status: LocalOmegonProbeStatus::PolicyDenied,
            policy,
            startup_url,
            state_url: None,
            instance_id: None,
            omegon_version: None,
            capabilities: Vec::new(),
            evidence: "policy denied local attach probe".into(),
            controller: None,
        }
    }

    fn failed(
        status: LocalOmegonProbeStatus,
        policy: PolicyDecision,
        startup_url: Option<String>,
        state_url: Option<String>,
        evidence: impl Into<String>,
    ) -> Self {
        Self {
            status,
            policy,
            startup_url,
            state_url,
            instance_id: None,
            omegon_version: None,
            capabilities: Vec::new(),
            evidence: evidence.into(),
            controller: None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn probe_local_omegon_candidate_read_only(
    candidate: &LocalOmegonCandidate,
    principal: PrincipalRef,
) -> LocalOmegonProbeResult {
    let startup_url = candidate.startup_url.clone();
    let resource = candidate
        .pid
        .map(|pid| runtime_resource(format!("pid:{pid}")))
        .unwrap_or_else(|| runtime_resource(startup_url.clone().unwrap_or_else(|| "unknown".into())));
    let policy = authorize_local_omegon_action(principal, LocalOmegonAction::Attach, resource);
    if !policy.is_allowed() {
        return LocalOmegonProbeResult::denied(policy, startup_url);
    }

    let Some(startup_url) = startup_url else {
        return LocalOmegonProbeResult::failed(
            LocalOmegonProbeStatus::MissingStartupUrl,
            policy,
            None,
            None,
            "candidate has no startup URL",
        );
    };

    let startup_body = match blocking_get_text(&startup_url) {
        Ok(body) => body,
        Err(error) => {
            return LocalOmegonProbeResult::failed(
                LocalOmegonProbeStatus::StartupFetchFailed,
                policy,
                Some(startup_url),
                None,
                error,
            );
        }
    };
    let startup: OmegonStartupInfo = match serde_json::from_str(&startup_body) {
        Ok(startup) => startup,
        Err(error) => {
            return LocalOmegonProbeResult::failed(
                LocalOmegonProbeStatus::StartupParseFailed,
                policy,
                Some(startup_url),
                None,
                format!("invalid startup payload: {error}"),
            );
        }
    };

    let state_url = startup_state_url(&startup).or_else(|| candidate.state_url.clone());
    let Some(state_url) = state_url else {
        return LocalOmegonProbeResult::failed(
            LocalOmegonProbeStatus::StateFetchFailed,
            policy,
            Some(startup_url),
            None,
            "startup payload did not provide state URL",
        );
    };

    let state_body = match blocking_get_text(&state_url) {
        Ok(body) => body,
        Err(error) => {
            return LocalOmegonProbeResult::failed(
                LocalOmegonProbeStatus::StateFetchFailed,
                policy,
                Some(startup_url),
                Some(state_url),
                error,
            );
        }
    };
    let mut controller = match crate::controller::AppController::from_remote_snapshot_json(&state_body) {
        Ok(controller) => controller,
        Err(error) => {
            return LocalOmegonProbeResult::failed(
                LocalOmegonProbeStatus::StateParseFailed,
                policy,
                Some(startup_url),
                Some(state_url),
                format!("invalid state payload: {error}"),
            );
        }
    };
    if let Some(record) = instance_record_from_startup_descriptor(&startup) {
        controller = controller.with_instance_registry(crate::instance_registry::InstanceRegistryStore {
            schema_version: 1,
            instances: vec![record],
        });
    }

    let descriptor = startup.instance_descriptor.as_ref();
    let instance_id = descriptor.map(|descriptor| descriptor.identity.instance_id.clone());
    let omegon_version = descriptor
        .and_then(|descriptor| descriptor.control_plane.as_ref())
        .and_then(|control_plane| control_plane.omegon_version.clone())
        .filter(|version| !version.is_empty());
    let capabilities = descriptor
        .and_then(|descriptor| descriptor.control_plane.as_ref())
        .map(|control_plane| control_plane.capabilities.clone())
        .unwrap_or_default();

    LocalOmegonProbeResult {
        status: LocalOmegonProbeStatus::AttachedReadOnly,
        policy,
        startup_url: Some(startup_url),
        state_url: Some(state_url),
        instance_id,
        omegon_version,
        capabilities,
        evidence: "startup/state probes succeeded; attached read-only projection".into(),
        controller: Some(controller),
    }
}


fn worker_role_from_descriptor(role: &str) -> crate::runtime_types::WorkerRole {
    match role.replace('_', "-").as_str() {
        "supervised-child" => crate::runtime_types::WorkerRole::SupervisedChild,
        "detached-service" => crate::runtime_types::WorkerRole::DetachedService,
        _ => crate::runtime_types::WorkerRole::PrimaryDriver,
    }
}

fn lifecycle_from_descriptor(status: &str) -> crate::runtime_types::WorkerLifecycleState {
    match status.replace('_', "-").as_str() {
        "busy" => crate::runtime_types::WorkerLifecycleState::Busy,
        "degraded" => crate::runtime_types::WorkerLifecycleState::Degraded,
        "lost" => crate::runtime_types::WorkerLifecycleState::Lost,
        "exited" => crate::runtime_types::WorkerLifecycleState::Exited,
        _ => crate::runtime_types::WorkerLifecycleState::Ready,
    }
}

fn instance_record_from_startup_descriptor(
    startup: &OmegonStartupInfo,
) -> Option<crate::runtime_types::InstanceRecord> {
    let descriptor = startup.instance_descriptor.as_ref()?;
    let control_plane = descriptor.control_plane.as_ref();
    let instance_id = if descriptor.identity.instance_id.is_empty() {
        "local-omegon".into()
    } else {
        descriptor.identity.instance_id.clone()
    };
    let now = "probe-local".to_string();
    let observed_control_plane = crate::runtime_types::ObservedControlPlane {
        schema_version: startup.schema_version,
        omegon_version: control_plane
            .and_then(|control_plane| control_plane.omegon_version.clone())
            .unwrap_or_default(),
        base_url: control_plane
            .and_then(|control_plane| control_plane.base_url.clone())
            .or_else(|| Some(startup.http_base.clone()).filter(|value| !value.is_empty()))
            .unwrap_or_default(),
        startup_url: control_plane
            .and_then(|control_plane| control_plane.startup_url.clone())
            .or_else(|| Some(startup.startup_url.clone()).filter(|value| !value.is_empty()))
            .unwrap_or_default(),
        health_url: control_plane
            .and_then(|control_plane| control_plane.health_url.clone())
            .or_else(|| Some(startup.health_url.clone()).filter(|value| !value.is_empty()))
            .unwrap_or_default(),
        ready_url: control_plane
            .and_then(|control_plane| control_plane.ready_url.clone())
            .or_else(|| Some(startup.ready_url.clone()).filter(|value| !value.is_empty()))
            .unwrap_or_default(),
        ws_url: control_plane
            .and_then(|control_plane| control_plane.ws_url.clone())
            .or_else(|| Some(startup.ws_url.clone()).filter(|value| !value.is_empty()))
            .unwrap_or_default(),
        acp_url: control_plane
            .and_then(|control_plane| control_plane.acp_url.clone())
            .or_else(|| startup.acp_url.clone().filter(|value| !value.is_empty())),
        auth_mode: control_plane
            .and_then(|control_plane| control_plane.auth_mode.clone())
            .or_else(|| Some(startup.auth_mode.clone()).filter(|value| !value.is_empty()))
            .unwrap_or_default(),
        token_ref: control_plane.and_then(|control_plane| control_plane.token_ref.clone()),
        transport_security: control_plane.and_then(|control_plane| control_plane.transport_security.clone()),
        mtls: control_plane.and_then(|control_plane| control_plane.mtls),
        last_ready_at: Some(now.clone()),
        ..Default::default()
    };
    let compatibility = crate::compatibility::assess_observed_control_plane(&observed_control_plane);
    let capabilities = control_plane
        .map(|control_plane| control_plane.capabilities.clone())
        .unwrap_or_default();

    Some(crate::runtime_types::InstanceRecord {
        schema_version: 1,
        identity: crate::runtime_types::WorkerIdentity {
            instance_id: instance_id.clone(),
            role: worker_role_from_descriptor(&descriptor.identity.role),
            profile: descriptor.identity.profile.clone(),
            status: lifecycle_from_descriptor(&descriptor.identity.status),
            created_at: now.clone(),
            updated_at: now.clone(),
        },
        ownership: crate::runtime_types::WorkerOwnership {
            owner_kind: crate::runtime_types::OwnerKind::External,
            owner_id: descriptor
                .session
                .as_ref()
                .and_then(|session| session.session_id.clone())
                .unwrap_or_else(|| "local-probe".into()),
            parent_instance_id: None,
        },
        desired: crate::runtime_types::DesiredWorkerState::default(),
        observed: crate::runtime_types::ObservedWorkerState {
            placement: crate::runtime_types::ObservedPlacement {
                placement_id: descriptor
                    .runtime
                    .as_ref()
                    .and_then(|runtime| runtime.placement_id.clone())
                    .unwrap_or_else(|| "local-process".into()),
                host: descriptor
                    .runtime
                    .as_ref()
                    .and_then(|runtime| runtime.host.clone())
                    .unwrap_or_else(|| "localhost".into()),
                pid: descriptor.runtime.as_ref().and_then(|runtime| runtime.pid),
                ..Default::default()
            },
            control_plane: observed_control_plane,
            health: crate::runtime_types::ObservedHealth {
                ready: true,
                last_seen_at: Some(now.clone()),
                freshness: Some(crate::runtime_types::InstanceFreshness::Fresh),
                ..Default::default()
            },
            exit: crate::runtime_types::ObservedExit::default(),
            compatibility: Some(compatibility),
            operational_profile: None,
            capabilities: Some(
                crate::capability_registry::InstanceCapabilitySnapshot::from_instance_descriptor_capabilities(
                    instance_id,
                    capabilities,
                ),
            ),
        },
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn blocking_get_text(url: &str) -> Result<String, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|error| format!("could not build HTTP client: {error}"))?
        .get(url)
        .send()
        .map_err(|error| format!("GET {url} failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("GET {url} returned error: {error}"))?
        .text()
        .map_err(|error| format!("could not read {url}: {error}"))
}

fn startup_state_url(startup: &OmegonStartupInfo) -> Option<String> {
    if !startup.state_url.is_empty() {
        return Some(startup.state_url.clone());
    }
    if !startup.startup_url.is_empty() {
        return Some(startup.startup_url.replace("/api/startup", "/api/state"));
    }
    if !startup.http_base.is_empty() {
        return Some(format!("{}/api/state", startup.http_base.trim_end_matches('/')));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_denied_attach_probe_returns_structured_result() {
        let candidate = LocalOmegonCandidate::known_control_port(7842);
        let result = probe_local_omegon_candidate_read_only(&candidate, PrincipalRef::anonymous());
        assert_eq!(result.status, LocalOmegonProbeStatus::PolicyDenied);
        assert!(result.controller.is_none());
    }

    #[test]
    fn missing_startup_url_returns_structured_result_after_policy_allows() {
        let mut candidate = LocalOmegonCandidate::known_control_port(7842);
        candidate.startup_url = None;
        let result = probe_local_omegon_candidate_read_only(
            &candidate,
            crate::authorization::attach_probe_principal(),
        );
        assert_eq!(result.status, LocalOmegonProbeStatus::MissingStartupUrl);
        assert!(result.controller.is_none());
    }
}
