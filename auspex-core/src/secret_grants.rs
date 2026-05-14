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
pub const SECRET_GRANT_STORE_SCHEMA_VERSION: u32 = 1;

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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretGrantStore {
    pub schema_version: u32,
    #[serde(default)]
    pub grants: Vec<SecretGrant>,
    #[serde(default)]
    pub leases: Vec<SecretLease>,
}

impl Default for SecretGrantStore {
    fn default() -> Self {
        Self {
            schema_version: SECRET_GRANT_STORE_SCHEMA_VERSION,
            grants: Vec::new(),
            leases: Vec::new(),
        }
    }
}

impl SecretGrantStore {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn find_grant(&self, grant_id: &str) -> Option<&SecretGrant> {
        self.grants.iter().find(|grant| grant.grant_id == grant_id)
    }

    pub fn find_lease(&self, lease_id: &str) -> Option<&SecretLease> {
        self.leases.iter().find(|lease| lease.lease_id == lease_id)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct FileSecretGrantBroker {
    path: std::path::PathBuf,
    clock: fn() -> u64,
    store: std::sync::Mutex<SecretGrantStore>,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileSecretGrantBroker {
    pub fn open(path: impl Into<std::path::PathBuf>) -> Result<Self, SecretGrantError> {
        Self::open_with_clock(path, current_epoch_seconds)
    }

    pub fn open_with_clock(
        path: impl Into<std::path::PathBuf>,
        clock: fn() -> u64,
    ) -> Result<Self, SecretGrantError> {
        let path = path.into();
        let store = load_store_or_default(&path)?;
        Ok(Self {
            path,
            clock,
            store: std::sync::Mutex::new(store),
        })
    }

    pub fn snapshot(&self) -> Result<SecretGrantStore, SecretGrantError> {
        Ok(self.lock_store()?.clone())
    }

    fn lock_store(&self) -> Result<std::sync::MutexGuard<'_, SecretGrantStore>, SecretGrantError> {
        self.store
            .lock()
            .map_err(|_| SecretGrantError::Backend("secret grant store lock poisoned".into()))
    }

    fn persist_locked(&self, store: &SecretGrantStore) -> Result<(), SecretGrantError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                SecretGrantError::Backend(format!(
                    "could not create secret grant store directory: {error}"
                ))
            })?;
        }
        let json = store
            .to_json_pretty()
            .map_err(|error| SecretGrantError::Backend(error.to_string()))?;
        std::fs::write(&self.path, json).map_err(|error| {
            SecretGrantError::Backend(format!("could not write secret grant store: {error}"))
        })
    }

    fn now(&self) -> u64 {
        (self.clock)()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl SecretGrantBroker for FileSecretGrantBroker {
    async fn create_grant(&self, grant: SecretGrant) -> Result<SecretGrant, SecretGrantError> {
        if grant.schema_version != SECRET_GRANT_SCHEMA_VERSION {
            return Err(SecretGrantError::InvalidRequest(format!(
                "unsupported secret grant schema version {}",
                grant.schema_version
            )));
        }
        if grant.grant_id.trim().is_empty() {
            return Err(SecretGrantError::InvalidRequest(
                "secret grant id must not be empty".into(),
            ));
        }
        if grant.principal.id.trim().is_empty() {
            return Err(SecretGrantError::InvalidRequest(
                "secret grant principal id must not be empty".into(),
            ));
        }

        let mut store = self.lock_store()?;
        if store.find_grant(&grant.grant_id).is_some() {
            return Err(SecretGrantError::InvalidRequest(format!(
                "secret grant {} already exists",
                grant.grant_id
            )));
        }

        store.grants.push(grant.clone());
        self.persist_locked(&store)?;
        Ok(grant)
    }

    async fn redeem_grant(
        &self,
        request: SecretRedeemRequest,
    ) -> Result<SecretRedeemResponse, SecretGrantError> {
        let mut store = self.lock_store()?;
        let grant_index = store
            .grants
            .iter()
            .position(|grant| grant.grant_id == request.grant_id)
            .ok_or_else(|| SecretGrantError::NotFound(request.grant_id.clone()))?;
        let grant = store.grants[grant_index].clone();

        validate_redeem_request(&grant, &request)?;

        let now = self.now();
        let lease = SecretLease {
            lease_id: next_lease_id(&store, &grant.grant_id),
            grant_id: grant.grant_id.clone(),
            principal: grant.principal.clone(),
            secret_refs: grant.secret_refs.clone(),
            status: SecretLeaseStatus::Active,
            issued_at: epoch_timestamp(now),
            expires_at: epoch_timestamp(now.saturating_add(grant.lease_policy.ttl_seconds)),
            renewable: grant.lease_policy.renewable,
            renewal_count: 0,
            backend_lease_ref: Some(format!("file://{}", self.path.display())),
            rotation_generation: None,
        };
        let seed_plan = SecretSeedPlan {
            plan_id: format!("seed_plan_{}", lease.lease_id),
            grant_id: grant.grant_id.clone(),
            delivery_mode: request.requested_delivery_mode,
            secret_refs: grant.secret_refs.clone(),
            bootstrap_bundle: None,
            backend_manifest: None,
        };

        store.grants[grant_index].status = SecretGrantStatus::Redeemed;
        store.leases.push(lease.clone());
        self.persist_locked(&store)?;

        Ok(SecretRedeemResponse { lease, seed_plan })
    }

    async fn renew_lease(&self, lease_id: &str) -> Result<SecretLease, SecretGrantError> {
        let mut store = self.lock_store()?;
        let lease_index = store
            .leases
            .iter()
            .position(|lease| lease.lease_id == lease_id)
            .ok_or_else(|| SecretGrantError::NotFound(lease_id.to_string()))?;
        let lease = store.leases[lease_index].clone();
        let grant = store
            .find_grant(&lease.grant_id)
            .cloned()
            .ok_or_else(|| SecretGrantError::NotFound(lease.grant_id.clone()))?;

        if lease.status != SecretLeaseStatus::Active {
            return Err(SecretGrantError::Denied(format!(
                "lease {lease_id} is not active"
            )));
        }
        if !lease.renewable || !grant.lease_policy.renewable {
            return Err(SecretGrantError::Denied(format!(
                "lease {lease_id} is not renewable"
            )));
        }
        if let Some(max_renewals) = grant.lease_policy.max_renewals
            && lease.renewal_count >= max_renewals
        {
            return Err(SecretGrantError::Denied(format!(
                "lease {lease_id} reached max renewals"
            )));
        }

        let now = self.now();
        let mut renewed = lease;
        renewed.renewal_count = renewed.renewal_count.saturating_add(1);
        renewed.expires_at = epoch_timestamp(now.saturating_add(grant.lease_policy.ttl_seconds));
        renewed.status = SecretLeaseStatus::Active;
        store.leases[lease_index] = renewed.clone();
        self.persist_locked(&store)?;

        Ok(renewed)
    }

    async fn revoke_lease(&self, lease_id: &str) -> Result<SecretLease, SecretGrantError> {
        let mut store = self.lock_store()?;
        let lease_index = store
            .leases
            .iter()
            .position(|lease| lease.lease_id == lease_id)
            .ok_or_else(|| SecretGrantError::NotFound(lease_id.to_string()))?;

        store.leases[lease_index].status = SecretLeaseStatus::Revoked;
        let revoked = store.leases[lease_index].clone();
        self.persist_locked(&store)?;

        Ok(revoked)
    }
}

fn validate_redeem_request(
    grant: &SecretGrant,
    request: &SecretRedeemRequest,
) -> Result<(), SecretGrantError> {
    if grant.status != SecretGrantStatus::Active {
        return Err(SecretGrantError::Denied(format!(
            "secret grant {} is not active",
            grant.grant_id
        )));
    }
    if grant.principal != request.principal {
        return Err(SecretGrantError::Denied(format!(
            "principal {} cannot redeem grant {}",
            request.principal.id, grant.grant_id
        )));
    }
    if !grant.allows_delivery_mode(request.requested_delivery_mode) {
        return Err(SecretGrantError::Denied(format!(
            "delivery mode {:?} is not allowed for grant {}",
            request.requested_delivery_mode, grant.grant_id
        )));
    }
    check_constraint(
        "agent instance",
        grant.constraints.agent_instance_id.as_deref(),
        request.observed.agent_instance_id.as_deref(),
    )?;
    check_constraint(
        "placement",
        grant.constraints.placement_id.as_deref(),
        request.observed.placement_id.as_deref(),
    )?;
    check_constraint(
        "package digest",
        grant.constraints.package_digest.as_deref(),
        request.observed.package_digest.as_deref(),
    )?;
    check_constraint(
        "image digest",
        grant.constraints.image_digest.as_deref(),
        request.observed.image_digest.as_deref(),
    )?;
    Ok(())
}

fn check_constraint(
    label: &str,
    expected: Option<&str>,
    observed: Option<&str>,
) -> Result<(), SecretGrantError> {
    match (expected, observed) {
        (Some(expected), Some(observed)) if expected == observed => Ok(()),
        (Some(expected), Some(observed)) => Err(SecretGrantError::Denied(format!(
            "{label} constraint mismatch: expected {expected}, observed {observed}"
        ))),
        (Some(expected), None) => Err(SecretGrantError::Denied(format!(
            "{label} constraint missing: expected {expected}"
        ))),
        (None, _) => Ok(()),
    }
}

fn next_lease_id(store: &SecretGrantStore, grant_id: &str) -> String {
    let count = store
        .leases
        .iter()
        .filter(|lease| lease.grant_id == grant_id)
        .count()
        + 1;
    format!("lease_{grant_id}_{count}")
}

fn epoch_timestamp(seconds: u64) -> String {
    format!("unix:{seconds}")
}

#[cfg(not(target_arch = "wasm32"))]
fn current_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_store_or_default(path: &std::path::Path) -> Result<SecretGrantStore, SecretGrantError> {
    match std::fs::read_to_string(path) {
        Ok(json) => SecretGrantStore::from_json(&json)
            .map_err(|error| SecretGrantError::Backend(error.to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(SecretGrantStore::default())
        }
        Err(error) => Err(SecretGrantError::Backend(format!(
            "could not read secret grant store: {error}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_CLOCK: AtomicU64 = AtomicU64::new(1_800_000_000);

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

    #[cfg(not(target_arch = "wasm32"))]
    fn fixed_clock() -> u64 {
        TEST_CLOCK.load(Ordering::SeqCst)
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

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn file_broker_creates_and_redeems_active_grant() {
        TEST_CLOCK.store(1_800_000_000, Ordering::SeqCst);
        let path = unique_temp_path("redeem");
        let broker = FileSecretGrantBroker::open_with_clock(&path, fixed_clock).unwrap();
        let mut grant = sample_grant(SecretGrantStatus::Active);
        grant.lease_policy = SecretLeasePolicy {
            ttl_seconds: 300,
            renewable: true,
            max_ttl_seconds: Some(900),
            max_renewals: Some(2),
        };
        grant.constraints.agent_instance_id = Some("omg_primary_01".into());
        grant.constraints.package_digest = Some("sha256:package".into());

        broker.create_grant(grant.clone()).await.unwrap();
        let response = broker
            .redeem_grant(SecretRedeemRequest {
                grant_id: grant.grant_id.clone(),
                principal: grant.principal.clone(),
                requested_delivery_mode: SecretDeliveryMode::PullAfterEnroll,
                observed: SecretRedeemObservation {
                    agent_instance_id: Some("omg_primary_01".into()),
                    package_digest: Some("sha256:package".into()),
                    ..Default::default()
                },
            })
            .await
            .unwrap();

        assert_eq!(response.lease.grant_id, "grant_01");
        assert_eq!(response.lease.expires_at, "unix:1800000300");
        assert_eq!(
            response.seed_plan.delivery_mode,
            SecretDeliveryMode::PullAfterEnroll
        );
        assert_eq!(
            broker
                .snapshot()
                .unwrap()
                .find_grant("grant_01")
                .unwrap()
                .status,
            SecretGrantStatus::Redeemed
        );

        let reopened = FileSecretGrantBroker::open_with_clock(&path, fixed_clock).unwrap();
        assert!(
            reopened
                .snapshot()
                .unwrap()
                .find_lease("lease_grant_01_1")
                .is_some()
        );

        let _ = std::fs::remove_file(path);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn file_broker_rejects_wrong_principal_and_constraint_mismatch() {
        let path = unique_temp_path("deny");
        let broker = FileSecretGrantBroker::open_with_clock(&path, fixed_clock).unwrap();
        let mut grant = sample_grant(SecretGrantStatus::Active);
        grant.constraints.image_digest = Some("sha256:expected".into());
        broker.create_grant(grant.clone()).await.unwrap();

        let wrong_principal = broker
            .redeem_grant(SecretRedeemRequest {
                grant_id: grant.grant_id.clone(),
                principal: SecretGrantPrincipal::spiffe("spiffe://styrene.dev/agents/other"),
                requested_delivery_mode: SecretDeliveryMode::PullAfterEnroll,
                observed: SecretRedeemObservation {
                    image_digest: Some("sha256:expected".into()),
                    ..Default::default()
                },
            })
            .await
            .unwrap_err();
        assert!(matches!(wrong_principal, SecretGrantError::Denied(_)));

        let mismatch = broker
            .redeem_grant(SecretRedeemRequest {
                grant_id: grant.grant_id,
                principal: grant.principal,
                requested_delivery_mode: SecretDeliveryMode::PullAfterEnroll,
                observed: SecretRedeemObservation {
                    image_digest: Some("sha256:wrong".into()),
                    ..Default::default()
                },
            })
            .await
            .unwrap_err();
        assert!(matches!(mismatch, SecretGrantError::Denied(_)));

        let _ = std::fs::remove_file(path);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn file_broker_renews_and_revokes_lease() {
        TEST_CLOCK.store(1_800_000_000, Ordering::SeqCst);
        let path = unique_temp_path("renew");
        let broker = FileSecretGrantBroker::open_with_clock(&path, fixed_clock).unwrap();
        let mut grant = sample_grant(SecretGrantStatus::Active);
        grant.lease_policy = SecretLeasePolicy {
            ttl_seconds: 120,
            renewable: true,
            max_ttl_seconds: None,
            max_renewals: Some(1),
        };
        broker.create_grant(grant.clone()).await.unwrap();
        let response = broker
            .redeem_grant(SecretRedeemRequest {
                grant_id: grant.grant_id,
                principal: grant.principal,
                requested_delivery_mode: SecretDeliveryMode::OpenBaoLease,
                observed: SecretRedeemObservation::default(),
            })
            .await
            .unwrap();

        TEST_CLOCK.store(1_800_000_060, Ordering::SeqCst);
        let renewed = broker.renew_lease(&response.lease.lease_id).await.unwrap();
        assert_eq!(renewed.renewal_count, 1);
        assert_eq!(renewed.expires_at, "unix:1800000180");

        let denied = broker
            .renew_lease(&response.lease.lease_id)
            .await
            .unwrap_err();
        assert!(matches!(denied, SecretGrantError::Denied(_)));

        let revoked = broker.revoke_lease(&response.lease.lease_id).await.unwrap();
        assert_eq!(revoked.status, SecretLeaseStatus::Revoked);

        let _ = std::fs::remove_file(path);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn unique_temp_path(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "auspex-secret-grants-{label}-{nanos}-{}.json",
            std::process::id()
        ))
    }
}
