# Provider Auth Projection Security

Auspex treats provider `auth.json` as a materialization format for secret
grants, not as a separate credential system. This document captures the current
security posture for projected OpenAI Codex and other provider credentials.

## Required Runtime Baseline

Projected provider auth requires the Omegon `0.23.x` runtime line. During the
0.23 release process, validate against the local build first, then against the
published digest-pinned OCI image before production rollout. That runtime:

- honors `OMEGON_AUTH_JSON_PATH` before the default desktop auth path;
- routes provider readers, writers, and legacy resolvers through the shared
  auth path;
- preserves `openai-codex` `accountId` during refresh write-back;
- registers projected `auth.json` access, refresh, and account identity values
  with runtime redaction;
- emits operator-safe guidance for read-only projection write-back failures.

Older Omegon images may silently ignore Auspex's mounted auth path and must not
be used for projected provider auth smoke tests.

## Projection Modes

| Mode | Use | Blast radius |
|---|---|---|
| `authJsonSecret` | Dev or narrow Kubernetes Secret projection | One pod can read one mounted provider bundle. Secret remains in etcd unless encrypted-at-rest is enabled. |
| Vault Agent / VSO / CSI | Production Kubernetes | Pod receives only rendered material. Vault/OpenBao audit and rotation own lifecycle. |
| Local keychain / encrypted store | Desktop and local detached agents | Compromise is local-user scoped. Keychain prompts and encrypted store policy apply. |
| Sealed bootstrap redemption | SSH/shuttle or nonstandard hosts | One-time bootstrap token plus mTLS enrollment limits replay and host-placement misuse. |
| `secretName` env projection | Legacy broad env injection | Highest blast radius; every key in the Secret becomes process environment and is inherited by child processes unless filtered. |

## Invariants

- Provider OAuth is not agent identity. Agent identity remains
  StyreneIdentity/mTLS/SPIFFE-style workload identity.
- `auth.json` contents are high-value secret material. Never log or display
  `access`, `refresh`, `accountId`, API keys, or raw file contents.
- Each deployed agent should receive the smallest useful provider bundle, not a
  shared all-provider file.
- Prefer read-only mounts. Refresh write-back must be treated as rotation
  guidance, not an excuse to make broad writable secret mounts.
- Use `AUSPEX_PRIMARY_AGENT_AUTH_JSON_SECRET` for the bootstrapped primary
  agent. Reserve `AUSPEX_PRIMARY_AGENT_SECRET` for broad runtime env values
  that cannot be represented as file projections.

## Known Residual Risks

- Kubernetes Secret projection still stores base64-encoded material in etcd.
  Production clusters should use Vault/OpenBao with VSO, Vault Agent, or CSI
  and ensure Kubernetes encryption-at-rest is enabled.
- A pod with arbitrary code execution can read its mounted provider bundle.
  Scope grants per agent and avoid reusing the same OAuth bundle across
  unrelated operational domains.
- Access tokens hydrated into process environment can be inherited by child
  processes. Omegon redaction reduces output leakage, but least-privilege
  grants and child environment filtering remain required.
- Read-only projected OAuth refresh can produce a new in-memory access token
  that is not persisted. Operators must rotate or reproject before the backing
  refresh material becomes stale.
- `accountId` is not a bearer token but is still account identity material; it
  must be redacted and kept out of telemetry.

## Validation Checklist

1. Confirm the running Omegon image is `0.23.x`, and production deployments are
   pinned by digest rather than mutable tag.
2. Confirm pod env contains `OMEGON_AUTH_JSON_PATH=/config/omegon/auth.json`.
3. Confirm the mounted Secret or Vault projection contains only key
   `auth.json` for the target provider bundle.
4. Confirm Auspex fleet/status output reports provider readiness by reference
   only, never by value.
5. Confirm logs redact access tokens, refresh tokens, and Codex account ids.
6. Prefer Vault/VSO/CSI for production agents; document every use of broad
   `secretName` env injection as an exception.
