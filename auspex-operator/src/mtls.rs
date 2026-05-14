//! mTLS for the fleet API using StyreneIdentity-derived certificates.
//!
//! The operator's StyreneID derives a self-signed CA keypair. Client
//! connections (Auspex desktop, web, other operators) must present a
//! certificate signed by this CA. The CA cert is distributed via a
//! ConfigMap so clients can discover and trust it.
//!
//! Certificate derivation uses the HKDF hierarchy:
//!   operator_root → HKDF("styrene-tls-ca-v1") → CA Ed25519 seed
//!   operator_root → HKDF("styrene-tls-server-v1") → server Ed25519 seed
//!
//! Ed25519 keys are used directly (not RSA/ECDSA) — this matches the
//! Styrene identity model and works with rustls.
//!
//! ## Why mTLS from StyreneIdentity?
//!
//! - Zero external PKI dependency — no cert-manager, no Let's Encrypt
//! - Deterministic — same operator root always produces same CA
//! - Rotates with the operator identity, not on a separate schedule
//! - Binds fleet API access to the Styrene trust hierarchy

use styrene_identity::{KeyDeriver, pubkey};
use tracing::info;

/// TLS key material derived from the operator's StyreneIdentity.
pub struct DerivedTlsMaterial {
    /// CA signing seed (Ed25519, 32 bytes). Used to sign client certs.
    pub ca_seed: [u8; 32],
    /// CA public key (Ed25519 verifying key).
    pub ca_pubkey: [u8; 32],
    /// Server seed (Ed25519, 32 bytes). The operator's own TLS key.
    pub server_seed: [u8; 32],
    /// Server public key.
    pub server_pubkey: [u8; 32],
}

const TLS_CA_LABEL: &str = "_tls-ca";
const TLS_SERVER_LABEL: &str = "_tls-server";
const TLS_CLIENT_PREFIX: &str = "_tls-client";

/// Derive TLS CA and server key material from an operator root secret.
///
/// The CA key signs client certificates. The server key is the operator's
/// own identity presented during TLS handshake. Both are deterministic
/// from the operator root — restarting the operator produces the same certs.
pub fn derive_tls_material(operator_root: &[u8; 32]) -> DerivedTlsMaterial {
    let deriver = KeyDeriver::new(operator_root);

    // Derive CA seed using a custom HKDF expand (not a standard KeyPurpose,
    // since TLS CA is specific to the operator, not a general identity key).
    // We use the agent key derivation with a reserved label.
    let ca_seed = deriver
        .derive_agent_key(TLS_CA_LABEL)
        .expect("reserved label should not fail validation");
    let ca_pubkey = pubkey::ed25519_verifying_key(&ca_seed);

    let server_seed = deriver
        .derive_agent_key(TLS_SERVER_LABEL)
        .expect("reserved label should not fail validation");
    let server_pubkey = pubkey::ed25519_verifying_key(&server_seed);

    info!(
        ca_pubkey = %hex_encode(ca_pubkey.as_bytes()),
        server_pubkey = %hex_encode(server_pubkey.as_bytes()),
        "derived TLS material from operator identity"
    );

    DerivedTlsMaterial {
        ca_seed,
        ca_pubkey: *ca_pubkey.as_bytes(),
        server_seed,
        server_pubkey: *server_pubkey.as_bytes(),
    }
}

/// Issue a client certificate seed for an Auspex client identified by label.
///
/// The client presents this seed's public key during mTLS handshake.
/// The operator verifies it was derived from its own CA.
pub fn derive_client_seed(operator_root: &[u8; 32], client_label: &str) -> [u8; 32] {
    let deriver = KeyDeriver::new(operator_root);
    deriver
        .derive_agent_key(&format!("{TLS_CLIENT_PREFIX}/{client_label}"))
        .expect("client label should not fail validation")
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

    #[test]
    fn derives_stable_distinct_tls_material() {
        let root = [7u8; 32];

        let first = derive_tls_material(&root);
        let second = derive_tls_material(&root);

        assert_eq!(first.ca_seed, second.ca_seed);
        assert_eq!(first.ca_pubkey, second.ca_pubkey);
        assert_eq!(first.server_seed, second.server_seed);
        assert_eq!(first.server_pubkey, second.server_pubkey);
        assert_ne!(first.ca_seed, first.server_seed);
        assert_ne!(first.ca_pubkey, first.server_pubkey);
    }

    #[test]
    fn derives_client_seed_from_label() {
        let root = [11u8; 32];

        let auspex = derive_client_seed(&root, "auspex");
        let control = derive_client_seed(&root, "control-plane");

        assert_eq!(auspex, derive_client_seed(&root, "auspex"));
        assert_ne!(auspex, control);
    }
}
