//! StyreneID provisioning for managed agents.
//!
//! Derives per-agent identity material from the operator's root secret
//! using the HKDF hierarchy defined in the styrene-identity spec:
//!
//!   agent_root = HKDF(operator_root, "styrene-agent-master-v1", agent_label)
//!
//! From the agent root, all protocol keys are derived deterministically:
//! - RNS signing (Ed25519) and encryption (X25519)
//! - WireGuard (Curve25519)
//! - SSH host key (Ed25519)
//!
//! The operator pre-authorizes the agent's public keys in the mesh policy
//! before the pod starts. When styrened boots, it derives the same keys
//! from the injected secret and is immediately admitted.

use k8s_openapi::api::core::v1::Secret;
use kube::api::Patch;
use kube::{Api, api::PatchParams};
use serde_json::json;
use sha2::Digest;
use styrene_identity::pki::{
    StyreneCertificateChain, StyreneCertificateProfile,
    derive_server_certificate_chain_with_profile,
};
use styrene_identity::signer::RootSecret;
use styrene_identity::{KeyDeriver, KeyPurpose, pubkey};
use tracing::info;
use zeroize::Zeroize;

use crate::crd::OmegonAgent;

/// Provision a StyreneID for a managed agent.
///
/// 1. Reads the operator's root secret from the k8s Secret.
/// 2. Derives the agent's root secret via two-level HKDF.
/// 3. Derives protocol-specific public keys for mesh policy pre-authorization.
/// 4. Creates a k8s Secret containing the agent's root secret.
/// 5. Returns the derived public keys for status reporting.
pub async fn provision_identity(
    client: &kube::Client,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
) -> Result<ProvisionedIdentity, IdentityError> {
    let identity_spec = agent
        .spec
        .identity
        .as_ref()
        .ok_or(IdentityError::NotConfigured)?;

    if !identity_spec.provision {
        return Err(IdentityError::NotConfigured);
    }

    // Validate spec fields before touching k8s API.
    if identity_spec.operator_secret.is_empty() {
        return Err(IdentityError::OperatorSecretMissing(
            "operator_secret must not be empty".into(),
        ));
    }
    if identity_spec.operator_secret_key.is_empty() {
        return Err(IdentityError::OperatorSecretMissing(
            "operator_secret_key must not be empty".into(),
        ));
    }

    // Include namespace in derivation label to prevent cross-namespace
    // identity collision when two CRDs share the same k8s name.
    let default_label = format!("{ns}/{name}");
    let derivation_label = identity_spec
        .derivation_label
        .as_deref()
        .unwrap_or(&default_label);

    // 1. Read the operator's root secret.
    let secrets_api: Api<Secret> = Api::namespaced(client.clone(), ns);
    let operator_secret = secrets_api
        .get(&identity_spec.operator_secret)
        .await
        .map_err(|e| {
            IdentityError::OperatorSecretMissing(format!(
                "could not read operator secret '{}': {e}",
                identity_spec.operator_secret
            ))
        })?;

    // Verify the operator secret carries the expected identity label.
    // This prevents a malicious CRD from reading arbitrary secrets by
    // pointing operator_secret at another service's secret.
    let secret_labels = operator_secret.metadata.labels.as_ref();
    let is_identity_secret = secret_labels
        .and_then(|l| l.get("styrene.sh/identity"))
        .is_some_and(|v| v == "operator");
    if !is_identity_secret {
        return Err(IdentityError::OperatorSecretMissing(format!(
            "secret '{}' is missing label styrene.sh/identity=operator — refusing to read",
            identity_spec.operator_secret
        )));
    }

    let root_bytes = operator_secret
        .data
        .as_ref()
        .and_then(|d| d.get(&identity_spec.operator_secret_key))
        .ok_or_else(|| {
            IdentityError::OperatorSecretMissing(format!(
                "key '{}' not found in operator secret",
                identity_spec.operator_secret_key
            ))
        })?;

    let root_array: [u8; 32] = root_bytes.0.as_slice().try_into().map_err(|_| {
        IdentityError::OperatorSecretMissing("root secret must be exactly 32 bytes".into())
    })?;
    let operator_root = RootSecret::new(root_array);

    // 2. Two-level HKDF via styrene-identity's KeyDeriver.
    //    operator_root → agent_master → per-agent root.
    let deriver = KeyDeriver::new(operator_root.as_bytes());

    let mut agent_root = deriver
        .derive_agent_key(derivation_label)
        .map_err(|e| IdentityError::DerivationFailed(format!("{e}")))?;

    // 3. Derive protocol public keys from the agent's root.
    //    The agent's styrened sidecar does the same derivation from the
    //    injected root secret — we compute public keys here so the
    //    operator can pre-authorize them in the mesh policy.
    let agent_deriver = KeyDeriver::new(&agent_root);

    // RNS signing: Ed25519 public key.
    let mut rns_signing_seed = agent_deriver.derive(KeyPurpose::Signing);
    let rns_signing_pubkey = pubkey::ed25519_verifying_key(&rns_signing_seed);
    rns_signing_seed.zeroize();

    // RNS encryption: X25519 public key.
    let mut rns_encryption_seed = agent_deriver.derive(KeyPurpose::RnsEncryption);
    let rns_encryption_pubkey = pubkey::x25519_public_key(&rns_encryption_seed);
    rns_encryption_seed.zeroize();

    // WireGuard: X25519 public key (Curve25519).
    let mut wireguard_seed = agent_deriver.derive(KeyPurpose::WireGuard);
    let wireguard_pubkey = pubkey::x25519_public_key(&wireguard_seed);
    wireguard_seed.zeroize();

    // RNS destination hash: truncated SHA-256(signing_pubkey || encryption_pubkey).
    let rns_dest_hash = rns_destination_hash(
        rns_signing_pubkey.as_bytes(),
        rns_encryption_pubkey.as_bytes(),
    );

    // Encode public keys for logging and status.
    let wg_pubkey_b64 = base64_encode(wireguard_pubkey.as_bytes());
    let rns_dest_hex = hex_encode(&rns_dest_hash);

    // 4. Create the agent identity Secret.
    let agent_secret_name = format!("{name}-styrene-id");
    let agent_secret = json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": &agent_secret_name,
            "namespace": ns,
            "ownerReferences": [crate::reconciler::owner_ref(agent)],
            "labels": {
                "styrene.sh/identity": "agent",
                "styrene.sh/agent": name,
            },
        },
        "type": "Opaque",
        "data": {
            // Base64-encoded agent root secret (32 bytes).
            // styrened reads this and derives all protocol keys locally.
            "root-secret": base64_encode(&agent_root),
            // Derivation label for audit trail.
            "derivation-label": base64_encode(derivation_label.as_bytes()),
        }
    });

    // Zeroize the agent root now that it's been encoded into the Secret JSON.
    agent_root.zeroize();

    secrets_api
        .patch(
            &agent_secret_name,
            &PatchParams::apply("auspex-operator"),
            &Patch::Apply(agent_secret),
        )
        .await
        .map_err(|e| IdentityError::SecretCreationFailed(e.to_string()))?;

    let control_tls = if crate::reconciler::control_plane_tls_enabled(agent) {
        Some(
            provision_control_tls_secret(&secrets_api, agent, ns, name, &operator_root)
                .await
                .map_err(|e| IdentityError::ControlTlsSecretCreationFailed(e.to_string()))?,
        )
    } else {
        None
    };

    info!(
        agent = %name,
        rns_dest = %rns_dest_hex,
        wg_pubkey = %wg_pubkey_b64,
        control_tls_secret = control_tls.as_ref().map(|tls| tls.secret_name.as_str()).unwrap_or("disabled"),
        "provisioned StyreneID"
    );

    Ok(ProvisionedIdentity {
        secret_name: agent_secret_name,
        rns_destination_hash: rns_dest_hex,
        wireguard_pubkey: wg_pubkey_b64,
        mesh_role: identity_spec.mesh_role.clone(),
        control_tls,
    })
}

/// Derived identity material for status reporting.
pub struct ProvisionedIdentity {
    pub secret_name: String,
    pub rns_destination_hash: String,
    pub wireguard_pubkey: String,
    pub mesh_role: String,
    pub control_tls: Option<ProvisionedControlTls>,
}

/// Derived control-plane TLS material for status/log reporting.
pub struct ProvisionedControlTls {
    pub secret_name: String,
    pub ca_fingerprint_sha256: String,
    pub server_fingerprint_sha256: String,
}

#[derive(Debug)]
pub enum IdentityError {
    NotConfigured,
    OperatorSecretMissing(String),
    DerivationFailed(String),
    SecretCreationFailed(String),
    ControlTlsSecretCreationFailed(String),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "identity provisioning not configured"),
            Self::OperatorSecretMissing(e) => write!(f, "operator secret: {e}"),
            Self::DerivationFailed(e) => write!(f, "key derivation failed: {e}"),
            Self::SecretCreationFailed(e) => write!(f, "secret creation failed: {e}"),
            Self::ControlTlsSecretCreationFailed(e) => {
                write!(f, "control-plane TLS secret creation failed: {e}")
            }
        }
    }
}

async fn provision_control_tls_secret(
    secrets_api: &Api<Secret>,
    agent: &OmegonAgent,
    ns: &str,
    name: &str,
    operator_root: &RootSecret,
) -> Result<ProvisionedControlTls, ControlTlsProvisionError> {
    let tls = crate::reconciler::resolved_control_tls(agent, name)
        .ok_or(ControlTlsProvisionError::NotConfigured)?;
    let ca_scope = control_tls_ca_scope(ns);
    let agent_label = format!("{ns}/{name}");
    let profile = StyreneCertificateProfile {
        ca_not_before_year: tls.validity.ca_not_before_year,
        ca_not_after_year: tls.validity.ca_not_after_year,
        leaf_not_before_year: tls.validity.leaf_not_before_year,
        leaf_not_after_year: tls.validity.leaf_not_after_year,
        ..StyreneCertificateProfile::default()
    }
    .with_profile(tls.profile.clone())
    .with_ca_epoch(tls.ca_epoch.clone())
    .with_leaf_epoch(tls.leaf_epoch.clone());
    let chain = derive_server_certificate_chain_with_profile(
        operator_root,
        &ca_scope,
        &agent_label,
        control_tls_subject_alt_names(ns, name),
        &profile,
    )?;
    let secret = control_tls_secret_manifest(agent, ns, &tls, &chain);

    secrets_api
        .patch(
            &tls.secret_name,
            &PatchParams::apply("auspex-operator"),
            &Patch::Apply(secret),
        )
        .await?;

    Ok(ProvisionedControlTls {
        secret_name: tls.secret_name,
        ca_fingerprint_sha256: chain.ca_fingerprint_sha256,
        server_fingerprint_sha256: chain.leaf.fingerprint_sha256,
    })
}

fn control_tls_secret_manifest(
    agent: &OmegonAgent,
    ns: &str,
    tls: &crate::reconciler::ResolvedControlTls,
    chain: &StyreneCertificateChain,
) -> serde_json::Value {
    let mut data = serde_json::Map::new();
    data.insert(
        tls.cert_key.clone(),
        json!(base64_encode(chain.cert_chain_pem().as_bytes())),
    );
    data.insert(
        tls.key_key.clone(),
        json!(base64_encode(chain.leaf.private_key_pem().as_bytes())),
    );
    if let Some(client_ca_key) = tls.client_ca_key.as_ref() {
        data.insert(
            client_ca_key.clone(),
            json!(base64_encode(chain.ca_bundle_pem().as_bytes())),
        );
    }

    json!({
        "apiVersion": "v1",
        "kind": "Secret",
        "metadata": {
            "name": tls.secret_name,
            "namespace": ns,
            "ownerReferences": [crate::reconciler::owner_ref(agent)],
            "labels": {
                "styrene.sh/control-plane-tls": "true",
                "styrene.sh/agent": agent.metadata.name.as_deref().unwrap_or(""),
            },
            "annotations": {
                "styrene.sh/ca-fingerprint-sha256": chain.ca_fingerprint_sha256,
                "styrene.sh/server-fingerprint-sha256": chain.leaf.fingerprint_sha256,
                "styrene.sh/certificate-uri": chain.leaf.uri_san,
                "styrene.sh/tls-profile": chain.profile.profile,
                "styrene.sh/tls-ca-epoch": chain.profile.ca_epoch,
                "styrene.sh/tls-leaf-epoch": chain.profile.leaf_epoch,
                "styrene.sh/tls-ca-validity": format!("{}-{}", chain.profile.ca_not_before_year, chain.profile.ca_not_after_year),
                "styrene.sh/tls-leaf-validity": format!("{}-{}", chain.profile.leaf_not_before_year, chain.profile.leaf_not_after_year),
            },
        },
        "type": "kubernetes.io/tls",
        "data": data,
    })
}

fn control_tls_ca_scope(ns: &str) -> String {
    format!("auspex-control/{ns}")
}

fn control_tls_subject_alt_names(ns: &str, name: &str) -> Vec<String> {
    vec![
        name.to_string(),
        format!("{name}.{ns}"),
        format!("{name}.{ns}.svc"),
        format!("{name}.{ns}.svc.cluster.local"),
    ]
}

#[derive(Debug, thiserror::Error)]
enum ControlTlsProvisionError {
    #[error("control-plane TLS is not configured")]
    NotConfigured,
    #[error("certificate derivation failed: {0}")]
    Certificate(#[from] styrene_identity::pki::StyrenePkiError),
    #[error("kubernetes secret patch failed: {0}")]
    Kubernetes(#[from] kube::Error),
}

/// RNS destination hash: truncated SHA-256(signing_pubkey || encryption_pubkey).
/// This matches the Reticulum identity hash computation.
fn rns_destination_hash(signing: &[u8], encryption: &[u8]) -> [u8; 16] {
    let mut hasher = sha2::Sha256::new();
    hasher.update(signing);
    hasher.update(encryption);
    let hash = hasher.finalize();
    let mut dest = [0u8; 16];
    dest.copy_from_slice(&hash[..16]);
    dest
}

/// Base64-encode bytes (standard alphabet, with padding).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((n >> 18) & 63) as usize] as char);
        result.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((n >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(n & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn hex_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len() * 2);
    for byte in data {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> OmegonAgent {
        serde_json::from_value(serde_json::json!({
            "apiVersion": "styrene.sh/v1alpha1",
            "kind": "OmegonAgent",
            "metadata": {
                "name": "secure-primary",
                "namespace": "omegon-agents"
            },
            "spec": {
                "agent": "styrene.secure-primary",
                "model": "anthropic:claude-sonnet-4-6",
                "role": "primary-driver",
                "mode": "daemon",
                "controlPlane": {
                    "tls": {
                        "enabled": true,
                        "secretName": "secure-primary-control-tls"
                    }
                }
            }
        }))
        .expect("valid OmegonAgent")
    }

    #[test]
    fn control_tls_manifest_uses_styrene_pki_material() {
        let agent = agent();
        let tls = crate::reconciler::resolved_control_tls(&agent, "secure-primary")
            .expect("resolved TLS");
        let root = RootSecret::new([0x42; 32]);
        let profile = StyreneCertificateProfile::default();
        let chain = derive_server_certificate_chain_with_profile(
            &root,
            &control_tls_ca_scope("omegon-agents"),
            "omegon-agents/secure-primary",
            control_tls_subject_alt_names("omegon-agents", "secure-primary"),
            &profile,
        )
        .expect("certificate chain");

        let manifest = control_tls_secret_manifest(&agent, "omegon-agents", &tls, &chain);

        assert_eq!(manifest["metadata"]["name"], "secure-primary-control-tls");
        assert_eq!(manifest["type"], "kubernetes.io/tls");
        assert_eq!(
            manifest["metadata"]["annotations"]["styrene.sh/ca-fingerprint-sha256"],
            chain.ca_fingerprint_sha256
        );
        assert_eq!(
            manifest["data"]["tls.crt"],
            base64_encode(chain.cert_chain_pem().as_bytes())
        );
        assert_eq!(
            manifest["data"]["tls.key"],
            base64_encode(chain.leaf.private_key_pem().as_bytes())
        );
        assert_eq!(
            manifest["data"]["ca.crt"],
            base64_encode(chain.ca_bundle_pem().as_bytes())
        );
        assert_eq!(
            manifest["metadata"]["annotations"]["styrene.sh/tls-profile"],
            "default"
        );
        assert_eq!(
            manifest["metadata"]["annotations"]["styrene.sh/tls-ca-epoch"],
            "0"
        );
        assert_eq!(
            manifest["metadata"]["annotations"]["styrene.sh/tls-leaf-epoch"],
            "0"
        );
        assert_eq!(
            manifest["metadata"]["annotations"]["styrene.sh/tls-ca-validity"],
            "2026-2036"
        );
        assert_eq!(
            manifest["metadata"]["annotations"]["styrene.sh/tls-leaf-validity"],
            "2026-2031"
        );
    }

    #[test]
    fn control_tls_subject_alt_names_cover_cluster_service_forms() {
        assert_eq!(
            control_tls_subject_alt_names("omegon-agents", "secure-primary"),
            vec![
                "secure-primary",
                "secure-primary.omegon-agents",
                "secure-primary.omegon-agents.svc",
                "secure-primary.omegon-agents.svc.cluster.local",
            ]
        );
    }
}
