# Operator Security Tiers

How the auspex-operator handles identity, secrets, and transport security across deployment contexts. Pick the tier that matches your threat model.

## Tier 1 — File-based (development, solo operators)

**Who this is for:** Local dev clusters, minikube, single-operator deployments where you're the only person with cluster access.

**Root secret storage:** k8s Secret (`styrene-operator-identity`), created manually or via `kubectl create secret`.

**Agent key derivation:** Operator reads root from k8s Secret, derives agent keys via HKDF in-process, writes derived keys back as k8s Secrets. Seeds are zeroized on the stack after use, but the root transits through the operator's memory.

**Provider keys (ANTHROPIC_API_KEY, etc.):** k8s Secret mounted as env vars via `secretRef`.

**Fleet API auth:** Bearer token from `AUSPEX_API_TOKEN` env var.

**Transport:** Plain HTTP `:8080` behind Ingress TLS termination.

```yaml
apiVersion: styrene.sh/v1alpha1
kind: OmegonAgent
metadata:
  name: my-agent
spec:
  agent: styrene.coding-agent
  model: anthropic:claude-sonnet-4-6
  mode: daemon
  secrets:
    secretName: my-agent-keys
  identity:
    provision: true
    securityTier: file
```

**Setup:**
```bash
# Generate operator root (32 random bytes)
dd if=/dev/urandom bs=32 count=1 2>/dev/null | \
  kubectl create secret generic styrene-operator-identity \
    --from-file=root-secret=/dev/stdin \
    -l styrene.sh/identity=operator

# Set API token
kubectl set env deployment/auspex-operator \
  AUSPEX_API_TOKEN=$(openssl rand -hex 32)
```

---

## Tier 2 — Vault-backed (production, team deployments)

**Who this is for:** Teams, shared clusters, CI/CD pipelines, anywhere secrets should be auditable and rotatable without redeploying.

**Root secret storage:** HashiCorp Vault at a configured path (e.g. `secret/data/styrene/operator-root`). The operator authenticates via k8s ServiceAccount JWT.

**Agent key derivation:** Operator reads root from Vault, derives agent keys, writes them to Vault (not k8s Secrets) at `{vault_agent_prefix}/{agent_name}`. Agent pods get their keys via the Vault Agent sidecar injector — secrets never touch the k8s Secret API or etcd.

**Provider keys:** Stored in Vault. Injected into agent pods via Vault Agent annotations. The operator never sees them — it just sets the right annotations on the pod template.

**Fleet API auth:** Bearer token from Vault-injected env. mTLS optional.

**Transport:** mTLS using StyreneIdentity-derived certificates (see below).

```yaml
apiVersion: styrene.sh/v1alpha1
kind: OmegonAgent
metadata:
  name: overnight-reviewer
spec:
  agent: styrene.coding-agent
  model: anthropic:claude-sonnet-4-6
  mode: cronjob
  schedule: "0 2 * * *"
  secrets:
    vault:
      role: auspex-operator
      secrets:
        - path: secret/data/agents/overnight-reviewer/provider-keys
          destination: /config/omegon/auth.json
          template: |
            {{- with secret "secret/data/agents/overnight-reviewer/provider-keys" -}}
            {
              "anthropic": { "api_key": "{{ .Data.data.ANTHROPIC_API_KEY }}" }
            }
            {{- end -}}
        - path: secret/data/agents/overnight-reviewer/github
          destination: /config/github-token
  identity:
    provision: true
    securityTier: vault
    vaultPath: secret/data/styrene/operator-root
    vaultAgentPrefix: secret/data/styrene/agents
```

**Setup:**
```bash
# Store operator root in Vault
vault kv put secret/styrene/operator-root \
  root-secret=@<(dd if=/dev/urandom bs=32 count=1 2>/dev/null | base64)

# Create k8s auth role for the operator
vault write auth/kubernetes/role/auspex-operator \
  bound_service_account_names=auspex-operator \
  bound_service_account_namespaces=agents \
  policies=auspex-operator \
  ttl=1h

# Policy: operator can read its root and write agent keys
vault policy write auspex-operator - <<EOF
path "secret/data/styrene/operator-root" {
  capabilities = ["read"]
}
path "secret/data/styrene/agents/*" {
  capabilities = ["create", "update", "read"]
}
path "secret/data/agents/*/provider-keys" {
  capabilities = ["read"]
}
EOF
```

**What changes vs Tier 1:**
- Root secret never in k8s etcd
- Agent keys never in k8s etcd
- Provider keys never in k8s etcd
- Full Vault audit log of every secret access
- Secret rotation via Vault lease/TTL, not k8s Secret edits

---

## Tier 3 — HSM-backed (high-security, compliance, "clean room")

**Who this is for:** Regulated environments, government, financial services, anywhere the root secret must never exist in software memory.

**Root secret storage:** Hardware security module — YubiKey 5 (FIDO2 hmac-secret), AWS CloudHSM, Azure Managed HSM, or any PKCS#11 device. The root never leaves the hardware boundary.

**Agent key derivation:** The operator sends an HKDF derivation request to the HSM. The HSM performs the derivation internally and returns only the derived agent key. The operator root secret is never extracted — the HSM holds it and performs all cryptographic operations.

**Provider keys:** Vault-backed (Tier 2), with Vault itself backed by HSM auto-unseal.

**Fleet API auth:** mTLS required. Client certificates issued by the HSM-derived CA.

**Transport:** mTLS with PQC hybrid (strongSwan IKEv2 + ML-KEM-768 for the WireGuard tunnel, Ed25519 + ML-DSA-65 hybrid for identity signing).

```yaml
apiVersion: styrene.sh/v1alpha1
kind: OmegonAgent
metadata:
  name: classified-reviewer
spec:
  agent: styrene.coding-agent
  model: anthropic:claude-sonnet-4-6
  mode: job
  secrets:
    vault:
      role: auspex-hsm-operator
      secrets:
        - path: secret/data/classified/provider-keys
          destination: /config/omegon/auth.json
  identity:
    provision: true
    securityTier: hsm
    hsmSlot: "pkcs11:token=styrene-operator;pin-source=/run/hsm/pin"
    mtls: true
    meshRole: admin
```

**What changes vs Tier 2:**
- Operator root never in RAM — HSM performs derivation
- Key ceremony required to initialize the HSM (M-of-N split, witnessed)
- Audit trail is hardware-attested (HSM audit log + Vault audit log)
- PQC-ready: ML-KEM-768 for key exchange, ML-DSA-65 for signing
- Recovery requires HSM + seed phrase (BIP-39, 24 words)

**Setup:**
```bash
# Initialize HSM with operator root (one-time ceremony)
# Requires M-of-N custodians present
styrene-tui --onboard-hsm \
  --slot "pkcs11:token=styrene-operator" \
  --ceremony split-key \
  --threshold 3 \
  --shares 5

# The HSM now holds the root. The operator connects via PKCS#11.
# Mount the HSM device/socket into the operator pod:
#   /dev/hidraw0 (YubiKey) or /run/cloudhsm/socket (CloudHSM)
```

---

## Decision Matrix

| Concern | Tier 1 (file) | Tier 2 (vault) | Tier 3 (hsm) |
|---------|:---:|:---:|:---:|
| Root in k8s etcd | yes | no | no |
| Root in operator RAM | yes | yes | **no** |
| Agent keys in etcd | yes | no | no |
| Provider keys in etcd | yes | no | no |
| Audit trail | k8s audit | Vault audit | Vault + HSM audit |
| Secret rotation | manual | Vault lease | HSM + Vault |
| PQC readiness | no | no | **yes** |
| Recovery | k8s backup | Vault + seed | HSM + seed + ceremony |
| Setup complexity | 5 min | 1 hour | 1 day + ceremony |
| Dependency | k8s | k8s + Vault | k8s + Vault + HSM |

## mTLS via StyreneIdentity

Available at Tier 2+ (optional at Tier 1). The operator derives a TLS CA from its root:

```
operator_root
  └─ HKDF("_tls-ca")      → CA Ed25519 seed → self-signed CA cert
  └─ HKDF("_tls-server")  → server Ed25519 seed → server cert (signed by CA)
  └─ HKDF("_tls-client/{label}") → per-client seed → client cert (signed by CA)
```

The CA cert is published via a ConfigMap (`auspex-operator-ca`). Clients (Auspex desktop, web, CLI) present their derived client cert during TLS handshake. The operator verifies it was signed by the CA.

This gives you mTLS with zero external PKI — no cert-manager, no ACME, no Let's Encrypt. The entire certificate chain is deterministic from the operator's StyreneIdentity. Rotate the identity, rotate all certs.

```yaml
# Enable mTLS on the operator
identity:
  provision: true
  securityTier: vault
  mtls: true

# Client gets its cert seed from the operator:
# GET /api/mtls/client-cert?label=auspex-desktop-chris
# Returns: { "seed": "<base64>", "ca_cert": "<pem>" }
# Client derives Ed25519 keypair, generates self-signed cert, connects with mTLS.
```

## Canonical identity posture

StyreneIdentity is the canonical cryptographic root-source for Auspex deployments. Keycloak is the default OIDC projection for human/operator sessions when an external OIDC provider is not supplied; it consumes StyreneIdentity-derived x509 identities rather than creating the underlying key material. Envoy Gateway is the edge enforcement point: it validates client certificates against the StyreneIdentity-derived CA, strips any incoming identity headers, and forwards only verified identity metadata to Keycloak and Auspex.

The direction of trust is therefore:

```text
StyreneIdentity enrollment
  -> x509 client certificate
  -> Envoy mTLS verification
  -> Keycloak x509 login / OIDC claims
  -> Auspex authorization
```

Do not invert this flow by creating a Keycloak user first and importing that key into StyreneIdentity. Keycloak stores a projection of the identity (`styrene_id`, groups, roles, certificate fingerprint); StyreneIdentity owns durable registration, derivation, device/workload certificates, and revocation semantics.

### Humans vs agents

Humans and agents share the StyreneIdentity substrate, but they use different session and authorization paths.

| Principal | Durable identity | Primary auth path | Authorization source | Keycloak projection |
|-----------|------------------|-------------------|----------------------|---------------------|
| Human | `human/<name>` | device cert -> Envoy mTLS -> Keycloak x509 -> OIDC | Keycloak groups/roles plus Auspex policy | default |
| Human device | `device/<human>/<device>` | mTLS client certificate | device allow/revoke policy | linked to human user |
| Agent | `agent/<name>` | pod/workload cert -> Envoy or mesh mTLS | Auspex agent policy / mesh policy | optional |
| Agent requiring JWT | `agent/<name>` | mTLS-bound token exchange or client assertion | Keycloak client roles plus Auspex policy | service-account client |
| Infrastructure service | `service/<name>` | service cert -> mTLS | infrastructure policy | optional |

Human enrollment creates both the StyreneIdentity record and the Keycloak projection:

```text
styrene identity enroll human/chris --device chris-macbook --project-to-keycloak
  -> StyreneIdentity: human/chris + device/chris-macbook cert
  -> Keycloak: user human/chris, styrene_id attribute, Auspex groups
```

Agent enrollment is normally driven by `OmegonAgent` reconciliation. The operator derives an agent root from the operator root, writes the per-agent identity material to the configured backend, and the agent/styrened sidecar derives its protocol keys locally at startup:

```text
OmegonAgent/calorium-chef
  -> Auspex derives agent/calorium-chef
  -> styrened derives RNS, WireGuard, and x509 keys
  -> agent authenticates to mesh/control-plane APIs by mTLS
```

Agents should not be forced through human login. Create a Keycloak service-account projection only when an agent must call an OIDC/JWT-protected service. Provider OAuth credentials such as `openai-codex` tokens remain separate workload credentials: Auspex may authorize `agent/<name>` to consume the current credential bundle, but the agent identity itself is StyreneIdentity/mTLS.

## Vault Secret Paths (Convention)

```
secret/data/styrene/
  ├── operator-root              # Operator root secret (32 bytes)
  └── agents/
      ├── overnight-reviewer/    # Per-agent derived key
      ├── discord-agent/
      └── rust-dev/

secret/data/agents/
  ├── overnight-reviewer/
  │   ├── provider-keys          # ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.
  │   └── github                 # GITHUB_TOKEN
  ├── discord-agent/
  │   ├── provider-keys
  │   └── discord                # VOX_DISCORD_BOT_TOKEN
  └── shared/
      └── provider-keys          # Shared across agents (fallback)
```
