---
id: styrene-secret-grant-architecture
title: "Styrene secret grants for Auspex-managed agents"
status: design target
parent: auspex-runtime-backends
tags:
  - secrets
  - identity
  - deployment
  - styrene
related:
  - operator-security-tiers
  - auspex-primary-coordinator
  - auspex-runtime-backends
  - nex-forge-package-lane
---

# Styrene secret grants for Auspex-managed agents

Auspex needs one secret posture for every agent it launches or supervises:

- Kubernetes pods seeded through Vault Secrets Operator or Vault Agent.
- OCI containers seeded through mounted config or runtime env.
- Local detached Omegon services.
- SSH/shuttle-deployed Omegon binaries on arbitrary hosts.
- Future styrened/fleet-deployed Omegon instances.

The durable boundary is not "which backend injected the bytes." The durable boundary is:

1. Auspex authorizes a bounded secret grant.
2. The target agent proves identity and placement.
3. The target receives only the secrets, leases, and capabilities its grant allows.
4. The target stores runtime secret material locally and reports readiness without echoing values.

## Styrene components to reuse

| Component | Use in Auspex-managed agent deployment | Notes |
|---|---|---|
| `styrene-identity` | Agent identity, deterministic per-agent keys, SSH key derivation, mTLS certificate derivation, enrollment proofs. | The local PKI work is the right substrate for WSS/mTLS and one-time enrollment. Stabilize it before treating it as a published dependency. |
| `styrene-secrets` | Agent-local encrypted secret store and resolver. | Good base for Omegon runtime secret storage. It needs grant import, lease metadata, and rotation/revocation semantics above the current get/set/list/delete surface. |
| `styrene-rbac` | Authorization policy for deployment and secret grants. | Add explicit capabilities such as `deploy.agent`, `secret.grant`, `secret.redeem`, `secret.rotate`, and `secret.revoke`. |
| `styrene-ipc` / `styrened` fleet APIs | Remote execution, profile application, and eventual typed deploy/seed operations for nodes already running Styrene. | Current fleet primitives can bootstrap via exec/apply. Add typed file transfer or deploy-binary operations before making this a primary path. |
| `styrene-content` | Signed package, binary, manifest, and bootstrap-bundle distribution. | Use for integrity and provenance. Do not use content transport as the secret value channel unless payloads are sealed to the target identity. |
| `styrene-mqtt` | Event fabric for deployment and secret lifecycle audit. | Publish facts like grant-created, grant-redeemed, rotation-started, rotation-complete. Never publish secret values. |
| `styrene-tunnel` | Optional secure data plane for remote hosts beyond WSS/SSH. | Useful once remote agents need long-lived high-bandwidth channels. Not required for the first secret-grant slice. |
| `styrene-forge` | Forge and package catalog discovery. | Useful for resolving package metadata from GitHub/Forgejo. It is not the deployment engine. |

## FOSS lifecycle delegation

Auspex should not own full secret lifecycle mechanics unless an adapter cannot cover a deployment mode.

| Concern | Preferred owner | Auspex responsibility |
|---|---|---|
| Workload identity | SPIFFE/SPIRE through `spiffe`, `spiffe-rustls`, and `spiffe-rustls-tokio` | Bind agent/runtime identity into `SecretGrantPrincipal`; fall back to `styrene-identity` where SPIRE is unavailable. |
| Secret leasing, renewal, revocation, dynamic credentials, audit | OpenBao through Vault-compatible APIs such as `vaultrs` | Create and revoke grants/policies/leases; project lease state into the Auspex UI and audit stream. |
| Kubernetes secret materialization | External Secrets Operator or Secrets Store CSI Driver | Render backend-specific manifests from `SecretSeedPlan`; do not copy values through the operator where avoidable. |
| Policy language | `cedar-policy` or `styrene-rbac` | Decide whether a coordinator/operator may attach a grant to an agent. Keep the decision auditable. |
| Sealed SSH/shuttle bootstrap | `age` plus OpenBao response wrapping where available | Create sealed bootstrap descriptors; deliver references or wrapped one-time tokens, not reusable long-lived values. |
| Local runtime secret storage | `styrene-secrets`, `secrecy`, and `zeroize` | Store only redeemed runtime material on the agent side; expose readiness by reference and generation only. |

This keeps Auspex out of the business of being a general-purpose secrets manager. Its durable ownership is orchestration: mapping agents, deployments, workflows, and operator approvals to backend-native secret lifecycle systems.

## Required Auspex object model

`SecretRef`

Identifies a secret without exposing its value.

- logical name
- backend kind: vault, vso, kubernetes-secret, local-store, env, external
- backend path or selector
- intended mount/import target
- sensitivity class
- rotation policy

`SecretGrant`

Authorizes a principal to receive one or more secret references.

- grant id
- target agent identity
- allowed `SecretRef` set
- lease TTL and renewal policy
- allowed delivery modes
- required package/image digest
- required runtime placement constraints
- approval and audit correlation ids

`SecretSeedPlan`

Backend-specific realization of a grant.

- Kubernetes VSO or Vault Agent injection
- Kubernetes Secret mount
- OCI bind mount or env injection
- local keychain/store import
- sealed bootstrap bundle
- pull-after-enroll redemption

`SealedBootstrapBundle`

Minimal bootstrap material for hosts where Auspex cannot rely on Kubernetes-native injection.

- agent id and expected role
- Auspex enrollment endpoint
- CA bundle and expected server identity
- package or binary digest
- one-time enrollment token
- optional sealed initial payload, encrypted to the target identity
- expiry and replay guard

`SecretLease`

Runtime handle for an agent's current access.

- lease id
- agent id
- grant id
- issued and expires timestamps
- renewable flag
- revoked flag
- rotation generation

## Non-standard SSH/shuttle deployment flow

This is the important path because it proves the model is not Kubernetes-only.

1. The operator or primary coordinator requests an Omegon deployment for a remote host.
2. Auspex derives or allocates a target identity through `styrene-identity`.
3. Auspex creates a `SecretGrant` scoped to that identity, package digest, role, and placement.
4. Auspex creates a `SealedBootstrapBundle` containing no reusable long-lived secret.
5. The embedded Omegon shuttle extension uses SSH to copy:
   - the Omegon binary or Nex-built package
   - the sealed bootstrap bundle
   - a small launch profile
6. Remote Omegon starts, validates its package digest, loads bootstrap metadata, and opens a local `styrene-secrets` store.
7. Remote Omegon enrolls over WSS/mTLS and redeems the one-time grant.
8. Auspex verifies identity, package digest, placement, RBAC policy, grant expiry, and replay state.
9. Auspex returns only the allowed secret material or references.
10. Remote Omegon imports the material into `styrene-secrets`, reports secret readiness, and starts the requested runtime role.
11. Auspex emits non-secret lifecycle events through the control plane and `styrene-mqtt`.

## Kubernetes happy path

Kubernetes remains the easiest backend, but it should be one realization of the same grant model:

1. `SecretGrant` is created for the target `OmegonAgent`.
2. The Kubernetes adapter renders the grant as Vault Secrets Operator, Vault Agent, or Secret mount config.
3. The pod starts with a workload identity and mTLS material.
4. Omegon still reports which `SecretRef` ids became ready.
5. Rotation and revocation update the same grant/lease state as SSH and local deployments.

This keeps the operator UX consistent: the operator attaches secret references to an agent, not backend-specific secret plumbing.

## Security invariants

- No raw long-lived secret values over RNS, MQTT, logs, or UI telemetry.
- No bearer-only secret redemption for deployed agents; use mTLS/WSS once identity exists.
- Bootstrap tokens are one-time, short-lived, bound to agent identity, package digest, and placement.
- Secret grants are explicit, auditable, revocable, and lease-bound.
- Agents report readiness by `SecretRef` id and generation, never by value.
- Package or image digest must be part of the grant decision for non-interactive deployment.
- Deployment, identity, secret, and workflow changes share an audit correlation id.

## Gaps to close

1. Stabilize and publish the `styrene-identity` PKI layer used for CA/server/client certificate derivation.
2. Add secret-grant data types in Auspex or a small shared Styrene crate.
3. Extend `styrene-secrets` with import-from-grant, lease metadata, rotation generation, and secure deletion semantics.
4. Add RBAC capabilities for deployment and secret grant operations.
5. Add Auspex broker endpoints:
   - `POST /api/agents/enroll`
   - `POST /api/secrets/grants`
   - `POST /api/secrets/redeem`
   - `POST /api/secrets/leases/{id}/renew`
   - `POST /api/secrets/leases/{id}/revoke`
6. Add shuttle deployment support for writing a sealed bootstrap bundle and validating first contact.
7. Add typed fleet deploy primitives in `styrene-ipc` for styrened-managed hosts.
8. Emit audit events through `styrene-mqtt` while keeping secret values out of the event fabric.

## First implementation slice

The smallest useful slice is SSH/shuttle plus local secret import:

1. Define `SecretRef`, `SecretGrant`, `SecretSeedPlan`, `SealedBootstrapBundle`, and `SecretLease` in `auspex-core`. Done in `auspex_core::secret_grants`.
2. Implement a file-backed grant broker for development. Done as `FileSecretGrantBroker`.
3. Teach the remote deployment path to write a sealed bootstrap bundle.
4. Teach Omegon to redeem the bundle over WSS/mTLS and import values into `styrene-secrets`.
5. Surface grant state in Auspex as references, readiness, lease status, and rotation generation.

Kubernetes VSO and Vault Agent then become adapters over the same objects rather than a separate conceptual model.

Implementation has started in `auspex_core::secret_grants`. The module is intentionally a control-plane schema and broker trait, not a secret engine. OpenBao/Vault-compatible leases, SPIFFE/SPIRE workload identity, Kubernetes External Secrets, and sealed bootstrap bundles should be implemented as adapters behind this shape.

`DesiredWorkerState` and `InstantiateRequest` now carry a `WorkerSecurityBinding` with optional principal, secret refs, grant ids, and seed-plan ids. That is the bridge from generic worker launch to auditable secret grants: every future deployment adapter should be able to answer which principal it launches and which secret references it expects to materialize.
