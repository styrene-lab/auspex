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
use kube::{Api, api::PatchParams};
use kube::api::Patch;
use serde_json::json;
use sha2::Digest;
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
        .map_err(|e| IdentityError::OperatorSecretMissing(format!(
            "could not read operator secret '{}': {e}",
            identity_spec.operator_secret
        )))?;

    // Verify the operator secret carries the expected identity label.
    // This prevents a malicious CRD from reading arbitrary secrets by
    // pointing operator_secret at another service's secret.
    let secret_labels = operator_secret
        .metadata
        .labels
        .as_ref();
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
        .ok_or_else(|| IdentityError::OperatorSecretMissing(format!(
            "key '{}' not found in operator secret",
            identity_spec.operator_secret_key
        )))?;

    let mut root_array: [u8; 32] = root_bytes
        .0
        .as_slice()
        .try_into()
        .map_err(|_| IdentityError::OperatorSecretMissing(
            "root secret must be exactly 32 bytes".into(),
        ))?;

    // 2. Two-level HKDF via styrene-identity's KeyDeriver.
    //    operator_root → agent_master → per-agent root.
    let deriver = KeyDeriver::new(&root_array);
    root_array.zeroize();

    let mut agent_root = deriver
        .derive_agent_key(derivation_label)
        .map_err(|e| IdentityError::DerivationFailed(format!("{e}")))?;

    // 3. Derive protocol public keys from the agent's root.
    //    The agent's styrened sidecar does the same derivation from the
    //    injected root secret — we compute public keys here so the
    //    operator can pre-authorize them in the mesh policy.
    let agent_deriver = KeyDeriver::new(&agent_root);

    // RNS signing: Ed25519 public key.
    let mut rns_signing_seed = agent_deriver.derive(KeyPurpose::RnsSigning);
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

    info!(
        agent = %name,
        rns_dest = %rns_dest_hex,
        wg_pubkey = %wg_pubkey_b64,
        "provisioned StyreneID"
    );

    Ok(ProvisionedIdentity {
        secret_name: agent_secret_name,
        rns_destination_hash: rns_dest_hex,
        wireguard_pubkey: wg_pubkey_b64,
        mesh_role: identity_spec.mesh_role.clone(),
    })
}

/// Derived identity material for status reporting.
pub struct ProvisionedIdentity {
    pub secret_name: String,
    pub rns_destination_hash: String,
    pub wireguard_pubkey: String,
    pub mesh_role: String,
}

#[derive(Debug)]
pub enum IdentityError {
    NotConfigured,
    OperatorSecretMissing(String),
    DerivationFailed(String),
    SecretCreationFailed(String),
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured => write!(f, "identity provisioning not configured"),
            Self::OperatorSecretMissing(e) => write!(f, "operator secret: {e}"),
            Self::DerivationFailed(e) => write!(f, "key derivation failed: {e}"),
            Self::SecretCreationFailed(e) => write!(f, "secret creation failed: {e}"),
        }
    }
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
