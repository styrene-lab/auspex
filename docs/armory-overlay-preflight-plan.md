# Armory Overlay Preflight Plan

Auspex should treat Armory packages as reusable capability intent, not as
cluster-ready runtime manifests. The missing bridge is an Auspex-owned deployment
overlay plus a preflight operation that proves what would be deployed before the
operator mutates Kubernetes.

## Phase 1: Overlay Store and Preflight

Status: implemented in the operator API.

- Store deployment overlays in a Kubernetes ConfigMap, defaulting to
  `auspex-armory-overlays` in the watched namespace.
- Keep overlay ids Kubernetes ConfigMap-safe: ASCII letters, numbers, `.`, `_`,
  and `-` only. Armory refs like `profile/security-review` remain package refs,
  not overlay ids.
- Expose CRUD over `/api/armory/overlays`.
- Expose `/api/armory/preflight` to combine package metadata, install plan,
  overlay posture, deploy overrides, namespace scope, and generated
  `OmegonAgent` manifest.
- Classify the generated OCI image reference and surface supply-chain posture:
  digest pinning, mutable tags, expected SBOM, expected signature, expected
  provenance, package artifact reference, payload digest, and verification
  command.
- Support strict OCI policy through request `ociPolicy: "strict"` or
  `AUSPEX_OCI_PREFLIGHT_POLICY=strict`; strict mode marks preflight blocked
  unless the generated image is digest-pinned.
- Do not pull OCI artifacts, install extensions, grant secrets, or apply CRDs in
  preflight.

## Phase 2: WebUI Review Path

The deployed WebUI should add an Armory deploy drawer that:

- Selects an Armory package and shows install plan gates.
- Selects or edits a deployment overlay.
- Shows required and optional secret names against `/api/secrets/grants`.
- Shows generated runtime posture: model, image, mode, role, TLS profile, mesh
  role, terminal tool posture, namespace, and expected connectors.
- Shows OCI image posture and makes mutable tags visually distinct from
  digest-pinned deployments.
- Shows blocked and approval-required policy gates before enabling deploy.

## Phase 3: Guarded Deploy

After review exists, add an Armory deploy route that consumes the same preflight
request and applies the previewed manifest. The route should reject drift between
preflight and apply by recomputing the manifest server-side.

Required guardrails:

- Deny namespace changes outside `AUSPEX_WATCH_NAMESPACE`.
- Require explicit acknowledgement for approval-required policy gates.
- Deny blocked gates.
- Record package ref, overlay id, plan hash, and operator identity in
  annotations.
- Never accept user-supplied Secret values through this API.

## Phase 4: Runtime Feedback

Post-deploy UI should follow the resulting agent lifecycle:

- CR accepted.
- Workload created.
- Pod ready.
- Control plane published.
- ACP proxy reachable.
- Secret envelopes visible and redacted.

This keeps the operator workflow legible: Armory describes what can be run,
Auspex chooses how it is run, and Kubernetes remains the source of truth for
what is actually running.
