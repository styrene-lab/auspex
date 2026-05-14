//! Secret-grant orchestration helpers for worker instantiation.
//!
//! This module bridges generic worker launch requests to the backend-agnostic
//! grant broker. Runtime adapters can call this before they start a worker so
//! the resulting request carries grant and seed-plan ids instead of raw
//! credential material.

use crate::runtime_types::{BackendKind, InstantiateRequest};
use crate::secret_grants::{
    SecretDeliveryMode, SecretGrant, SecretGrantBroker, SecretGrantConstraints, SecretGrantError,
    SecretGrantPrincipal, SecretGrantStatus, SecretLease, SecretLeasePolicy,
    SecretRedeemObservation, SecretRedeemRequest, SecretSeedPlan,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedWorkerSecurity {
    pub request: InstantiateRequest,
    pub grant: SecretGrant,
    pub seed_plan: SecretSeedPlan,
    pub lease: SecretLease,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrepareWorkerSecurityOptions {
    pub delivery_mode: SecretDeliveryMode,
    pub lease_policy: SecretLeasePolicy,
    pub grant_id: Option<String>,
    pub principal: Option<SecretGrantPrincipal>,
    pub package_digest: Option<String>,
    pub image_digest: Option<String>,
    pub placement_id: Option<String>,
    pub audit_correlation_id: Option<String>,
    pub requested_by: Option<String>,
    pub approved_by: Option<String>,
    pub reason: Option<String>,
}

impl Default for PrepareWorkerSecurityOptions {
    fn default() -> Self {
        Self {
            delivery_mode: SecretDeliveryMode::PullAfterEnroll,
            lease_policy: SecretLeasePolicy::default(),
            grant_id: None,
            principal: None,
            package_digest: None,
            image_digest: None,
            placement_id: None,
            audit_correlation_id: None,
            requested_by: None,
            approved_by: None,
            reason: None,
        }
    }
}

pub async fn prepare_worker_security(
    broker: &dyn SecretGrantBroker,
    request: InstantiateRequest,
    options: PrepareWorkerSecurityOptions,
) -> Result<PreparedWorkerSecurity, SecretGrantError> {
    if request.security.secret_refs.is_empty() {
        return Err(SecretGrantError::InvalidRequest(
            "worker request has no secret refs to grant".into(),
        ));
    }

    let principal = options
        .principal
        .clone()
        .or_else(|| request.security.principal.clone())
        .unwrap_or_else(|| default_principal_for_request(&request));
    let grant_id = options
        .grant_id
        .clone()
        .unwrap_or_else(|| default_grant_id(&request));

    let mut grant = SecretGrant::new(grant_id, principal.clone());
    grant.secret_refs = request.security.secret_refs.clone();
    grant.allowed_delivery_modes = vec![options.delivery_mode];
    grant.constraints = SecretGrantConstraints {
        agent_instance_id: None,
        role: Some(format!("{:?}", request.role).to_ascii_lowercase()),
        backend: Some(format!("{:?}", request.backend).to_ascii_lowercase()),
        placement_id: options.placement_id.clone(),
        image_digest: options.image_digest.clone(),
        package_digest: options.package_digest.clone(),
    };
    grant.lease_policy = options.lease_policy.clone();
    grant.approval.requested_by = options.requested_by.clone();
    grant.approval.approved_by = options.approved_by.clone();
    grant.approval.audit_correlation_id = options.audit_correlation_id.clone();
    grant.approval.reason = options.reason.clone();
    grant.status = SecretGrantStatus::Active;

    let grant = broker.create_grant(grant).await?;
    let redeem_response = broker
        .redeem_grant(SecretRedeemRequest {
            grant_id: grant.grant_id.clone(),
            principal: principal.clone(),
            requested_delivery_mode: options.delivery_mode,
            observed: SecretRedeemObservation {
                placement_id: options.placement_id,
                package_digest: options.package_digest,
                image_digest: options.image_digest,
                ..Default::default()
            },
        })
        .await?;

    let mut request = request;
    request.security.principal = Some(principal);
    request.security.grant_ids.push(grant.grant_id.clone());
    request
        .security
        .seed_plan_ids
        .push(redeem_response.seed_plan.plan_id.clone());

    Ok(PreparedWorkerSecurity {
        request,
        grant,
        seed_plan: redeem_response.seed_plan,
        lease: redeem_response.lease,
    })
}

pub fn default_secret_grant_store_path() -> Option<std::path::PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let root = std::env::current_dir().ok()?;
        Some(root.join(".auspex").join("secret-grants.json"))
    }
}

fn default_principal_for_request(request: &InstantiateRequest) -> SecretGrantPrincipal {
    let role = format!("{:?}", request.role).to_ascii_lowercase();
    let workspace = sanitize_principal_component(&request.workspace.workspace_id);
    SecretGrantPrincipal::styrene_identity(format!("styrene://auspex/{role}/{workspace}"))
}

fn default_grant_id(request: &InstantiateRequest) -> String {
    let role = format!("{:?}", request.role).to_ascii_lowercase();
    let profile = sanitize_principal_component(&request.profile);
    let workspace = sanitize_principal_component(&request.workspace.workspace_id);
    format!("grant_{role}_{profile}_{workspace}")
}

fn sanitize_principal_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_');
    if trimmed.is_empty() {
        "unknown".into()
    } else {
        trimmed.into()
    }
}

pub fn default_delivery_mode_for_backend(backend: BackendKind) -> SecretDeliveryMode {
    match backend {
        BackendKind::Kubernetes => SecretDeliveryMode::KubernetesExternalSecret,
        BackendKind::OciContainer => SecretDeliveryMode::LocalStoreImport,
        BackendKind::LocalProcess | BackendKind::LocalDetached => {
            SecretDeliveryMode::PullAfterEnroll
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_types::{
        BackendKind, InstantiateRequest, TaskBinding, WorkerRole, WorkspaceBinding,
    };
    use crate::secret_grants::FileSecretGrantBroker;
    use pretty_assertions::assert_eq;

    fn request_with_secret_refs() -> InstantiateRequest {
        InstantiateRequest {
            schema_version: 1,
            role: WorkerRole::SupervisedChild,
            profile: "cheap-subtask".into(),
            backend: BackendKind::LocalDetached,
            workspace: WorkspaceBinding {
                cwd: "/repo".into(),
                workspace_id: "repo:demo".into(),
                branch: Some("main".into()),
            },
            parent_instance_id: Some("omg_primary".into()),
            task: Some(TaskBinding {
                task_id: "task-1".into(),
                purpose: "test".into(),
                spec_binding: None,
            }),
            security: crate::runtime_types::WorkerSecurityBinding {
                secret_refs: vec!["provider.anthropic".into()],
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn prepare_worker_security_creates_grant_and_updates_request() {
        let path = unique_temp_path("prepare");
        let broker = FileSecretGrantBroker::open(&path).unwrap();
        let request = request_with_secret_refs();

        let prepared = prepare_worker_security(
            &broker,
            request,
            PrepareWorkerSecurityOptions {
                grant_id: Some("grant_test".into()),
                principal: Some(SecretGrantPrincipal::spiffe(
                    "spiffe://styrene.dev/agents/task-1",
                )),
                delivery_mode: SecretDeliveryMode::PullAfterEnroll,
                package_digest: Some("sha256:pkg".into()),
                reason: Some("unit test".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(prepared.grant.grant_id, "grant_test");
        assert_eq!(prepared.lease.grant_id, "grant_test");
        assert_eq!(
            prepared.seed_plan.delivery_mode,
            SecretDeliveryMode::PullAfterEnroll
        );
        assert_eq!(
            prepared.request.security.principal.as_ref().unwrap().id,
            "spiffe://styrene.dev/agents/task-1"
        );
        assert_eq!(prepared.request.security.grant_ids, vec!["grant_test"]);
        assert_eq!(
            prepared.request.security.seed_plan_ids,
            vec!["seed_plan_lease_grant_test_1"]
        );

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn prepare_worker_security_rejects_request_without_secret_refs() {
        let path = unique_temp_path("missing");
        let broker = FileSecretGrantBroker::open(&path).unwrap();
        let mut request = request_with_secret_refs();
        request.security.secret_refs.clear();

        let error =
            prepare_worker_security(&broker, request, PrepareWorkerSecurityOptions::default())
                .await
                .unwrap_err();

        assert!(matches!(error, SecretGrantError::InvalidRequest(_)));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn default_delivery_mode_matches_backend_expectations() {
        assert_eq!(
            default_delivery_mode_for_backend(BackendKind::Kubernetes),
            SecretDeliveryMode::KubernetesExternalSecret
        );
        assert_eq!(
            default_delivery_mode_for_backend(BackendKind::LocalDetached),
            SecretDeliveryMode::PullAfterEnroll
        );
    }

    fn unique_temp_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "auspex-secret-orchestration-{label}-{nanos}-{}.json",
            std::process::id()
        ))
    }
}
