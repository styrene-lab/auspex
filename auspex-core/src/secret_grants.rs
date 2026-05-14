//! Backend-agnostic secret grant types for Auspex-managed agents.
//!
//! Auspex should coordinate secret access without becoming the secret engine.
//! These types describe the control-plane contract that adapters can realize
//! through OpenBao/Vault, SPIFFE/SPIRE, Kubernetes External Secrets, local
//! stores, or sealed bootstrap bundles.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub const SECRET_GRANT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRef {
    pub id: String,
    pub display_name: String,
    pub backend: SecretBackendRef,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub selector: BTreeMap<String, String>,
    #[serde(default)]
    pub target: SecretTarget,
    #[serde(default)]
    pub sensitivity: SecretSensitivity,
    #[serde(default)]
    pub rotation: SecretRotationPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretBackendRef {
    pub kind: SecretBackendKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mount: Option<String>,
    pub path: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretBackendKind {
    OpenBao,
    VaultCompatible,
    ExternalSecretsOperator,
    SecretsStoreCsi,
    KubernetesSecret,
    StyreneSecrets,
    LocalKeyring,
    Environment,
    #[default]
    External,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretTarget {
    #[serde(default)]
    pub mode: SecretTargetMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretTargetMode {
    File,
    Env,
    AgentStore,
    #[default]
    ReferenceOnly,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretSensitivity {
    PublicReference,
    Internal,
    Credential,
    HighValue,
    #[default]
    Secret,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRotationPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotate_after_seconds: Option<u64>,
    #[serde(default)]
    pub rotate_on_next_reconcile: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretGrant {
    pub schema_version: u32,
    pub grant_id: String,
    pub principal: SecretGrantPrincipal,
    #[serde(default)]
    pub secret_refs: Vec<String>,
    #[serde(default)]
    pub allowed_delivery_modes: Vec<SecretDeliveryMode>,
    #[serde(default)]
    pub constraints: SecretGrantConstraints,
    #[serde(default)]
    pub lease_policy: SecretLeasePolicy,
    #[serde(default)]
    pub approval: SecretGrantApproval,
    #[serde(default)]
    pub status: SecretGrantStatus,
}

impl SecretGrant {
    pub fn new(grant_id: impl Into<String>, principal: SecretGrantPrincipal) -> Self {
        Self {
            schema_version: SECRET_GRANT_SCHEMA_VERSION,
            grant_id: grant_id.into(),
            principal,
            secret_refs: Vec::new(),
            allowed_delivery_modes: Vec::new(),
            constraints: SecretGrantConstraints::default(),
            lease_policy: SecretLeasePolicy::default(),
            approval: SecretGrantApproval::default(),
            status: SecretGrantStatus::Pending,
        }
    }

    pub fn allows_secret_ref(&self, secret_ref_id: &str) -> bool {
        self.status == SecretGrantStatus::Active
            && self.secret_refs.iter().any(|id| id == secret_ref_id)
    }

    pub fn allows_delivery_mode(&self, mode: SecretDeliveryMode) -> bool {
        self.status == SecretGrantStatus::Active
            && self
                .allowed_delivery_modes
                .iter()
                .any(|allowed| *allowed == mode)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretGrantPrincipal {
    pub kind: SecretPrincipalKind,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

impl SecretGrantPrincipal {
    pub fn spiffe(id: impl Into<String>) -> Self {
        Self {
            kind: SecretPrincipalKind::Spiffe,
            id: id.into(),
            display_name: None,
        }
    }

    pub fn styrene_identity(id: impl Into<String>) -> Self {
        Self {
            kind: SecretPrincipalKind::StyreneIdentity,
            id: id.into(),
            display_name: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretPrincipalKind {
    Spiffe,
    StyreneIdentity,
    KubernetesServiceAccount,
    SshHost,
    OidcSubject,
    Opaque,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretDeliveryMode {
    OpenBaoLease,
    VaultCompatibleLease,
    KubernetesExternalSecret,
    KubernetesSecretMount,
    SecretsStoreCsiMount,
    LocalStoreImport,
    SealedBootstrapBundle,
    PullAfterEnroll,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretGrantConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_digest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretLeasePolicy {
    pub ttl_seconds: u64,
    #[serde(default)]
    pub renewable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ttl_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_renewals: Option<u32>,
}

impl Default for SecretLeasePolicy {
    fn default() -> Self {
        Self {
            ttl_seconds: 900,
            renewable: false,
            max_ttl_seconds: None,
            max_renewals: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretGrantApproval {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretGrantStatus {
    #[default]
    Pending,
    Active,
    Redeemed,
    Expired,
    Revoked,
    Denied,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretSeedPlan {
    pub plan_id: String,
    pub grant_id: String,
    pub delivery_mode: SecretDeliveryMode,
    #[serde(default)]
    pub secret_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_bundle: Option<SealedBootstrapBundleDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_manifest: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SealedBootstrapBundleDescriptor {
    pub bundle_id: String,
    pub agent_instance_id: String,
    pub enrollment_endpoint: String,
    pub ca_bundle_ref: String,
    pub expected_server_identity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_digest: Option<String>,
    /// Reference to a wrapped one-time token. This must not be the token value.
    pub one_time_token_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed_payload_ref: Option<String>,
    pub expires_at: String,
    pub replay_guard: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretLease {
    pub lease_id: String,
    pub grant_id: String,
    pub principal: SecretGrantPrincipal,
    #[serde(default)]
    pub secret_refs: Vec<String>,
    pub status: SecretLeaseStatus,
    pub issued_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub renewable: bool,
    #[serde(default)]
    pub renewal_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_lease_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation_generation: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecretLeaseStatus {
    Active,
    Renewing,
    Expired,
    Revoked,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRedeemRequest {
    pub grant_id: String,
    pub principal: SecretGrantPrincipal,
    pub requested_delivery_mode: SecretDeliveryMode,
    #[serde(default)]
    pub observed: SecretRedeemObservation,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRedeemObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_digest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRedeemResponse {
    pub lease: SecretLease,
    pub seed_plan: SecretSeedPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretGrantError {
    NotFound(String),
    Denied(String),
    InvalidRequest(String),
    Backend(String),
}

impl std::fmt::Display for SecretGrantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(message) => write!(f, "secret grant not found: {message}"),
            Self::Denied(message) => write!(f, "secret grant denied: {message}"),
            Self::InvalidRequest(message) => write!(f, "invalid secret grant request: {message}"),
            Self::Backend(message) => write!(f, "secret grant backend error: {message}"),
        }
    }
}

impl std::error::Error for SecretGrantError {}

#[async_trait]
pub trait SecretGrantBroker: Send + Sync {
    async fn create_grant(&self, grant: SecretGrant) -> Result<SecretGrant, SecretGrantError>;

    async fn redeem_grant(
        &self,
        request: SecretRedeemRequest,
    ) -> Result<SecretRedeemResponse, SecretGrantError>;

    async fn renew_lease(&self, lease_id: &str) -> Result<SecretLease, SecretGrantError>;

    async fn revoke_lease(&self, lease_id: &str) -> Result<SecretLease, SecretGrantError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sample_grant(status: SecretGrantStatus) -> SecretGrant {
        let mut grant = SecretGrant::new(
            "grant_01",
            SecretGrantPrincipal::spiffe("spiffe://styrene.dev/agents/primary-driver"),
        );
        grant.secret_refs = vec!["provider.anthropic".into(), "github.release".into()];
        grant.allowed_delivery_modes = vec![
            SecretDeliveryMode::OpenBaoLease,
            SecretDeliveryMode::PullAfterEnroll,
        ];
        grant.status = status;
        grant
    }

    #[test]
    fn active_grant_allows_declared_secret_and_delivery_mode() {
        let grant = sample_grant(SecretGrantStatus::Active);

        assert!(grant.allows_secret_ref("provider.anthropic"));
        assert!(grant.allows_delivery_mode(SecretDeliveryMode::PullAfterEnroll));
        assert!(!grant.allows_secret_ref("provider.openai"));
        assert!(!grant.allows_delivery_mode(SecretDeliveryMode::SealedBootstrapBundle));
    }

    #[test]
    fn inactive_grant_allows_nothing() {
        let grant = sample_grant(SecretGrantStatus::Pending);

        assert!(!grant.allows_secret_ref("provider.anthropic"));
        assert!(!grant.allows_delivery_mode(SecretDeliveryMode::PullAfterEnroll));
    }

    #[test]
    fn secret_ref_round_trips_as_reference_not_value() {
        let secret_ref = SecretRef {
            id: "provider.anthropic".into(),
            display_name: "Anthropic provider key".into(),
            backend: SecretBackendRef {
                kind: SecretBackendKind::OpenBao,
                address: Some("https://bao.internal:8200".into()),
                namespace: None,
                mount: Some("secret".into()),
                path: "agents/primary-driver/provider".into(),
            },
            selector: BTreeMap::from([("json_key".into(), "ANTHROPIC_API_KEY".into())]),
            target: SecretTarget {
                mode: SecretTargetMode::AgentStore,
                destination: Some("omegon/providers/anthropic".into()),
                env_key: None,
            },
            sensitivity: SecretSensitivity::Credential,
            rotation: SecretRotationPolicy {
                generation: Some(3),
                rotate_after_seconds: Some(86_400),
                rotate_on_next_reconcile: false,
            },
        };

        let json = serde_json::to_string(&secret_ref).unwrap();

        assert!(!json.contains("sk-ant"));
        assert_eq!(
            serde_json::from_str::<SecretRef>(&json).unwrap(),
            secret_ref
        );
    }

    #[test]
    fn sealed_bootstrap_descriptor_carries_token_reference_only() {
        let descriptor = SealedBootstrapBundleDescriptor {
            bundle_id: "bundle_01".into(),
            agent_instance_id: "omg_remote_01".into(),
            enrollment_endpoint: "wss://auspex.example.test/api/agents/enroll".into(),
            ca_bundle_ref: "config://auspex/ca".into(),
            expected_server_identity: "spiffe://styrene.dev/auspex/control-plane".into(),
            package_digest: Some("sha256:abc123".into()),
            image_digest: None,
            one_time_token_ref: "openbao:wrapping-token:bundle_01".into(),
            sealed_payload_ref: Some("age:bundle_01.payload".into()),
            expires_at: "2026-05-14T17:00:00Z".into(),
            replay_guard: "nonce_01".into(),
        };

        let json = serde_json::to_string(&descriptor).unwrap();

        assert!(json.contains("one_time_token_ref"));
        assert!(!json.contains("one_time_token_value"));
        assert_eq!(
            serde_json::from_str::<SealedBootstrapBundleDescriptor>(&json).unwrap(),
            descriptor
        );
    }
}
