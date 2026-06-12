# Deployed WebUI Control Plane

The Kubernetes-deployed Auspex WebUI is the fleet control surface for managed
Omegon instances. It is not the desktop application running in a browser.

The desktop app is operator-local and session-oriented. The deployed WebUI is
cluster-scoped and lifecycle-oriented: it reads Kubernetes desired/observed
state, exposes control-plane endpoints, and issues guarded mutations against
`OmegonAgent` resources.

## API Surface

All routes live under `/api` and are protected by `AUSPEX_API_TOKEN` when set.
Deployments should set this token or front the service with OIDC/mTLS ingress.
Normal HTTP API calls use `Authorization: Bearer <token>`. Browser WebSocket
clients may pass the same token as `access_token` or `token` query parameter,
but only on WebSocket upgrade requests.

| Route | Method | Purpose |
|-------|--------|---------|
| `/api/fleet` | `GET` | List managed and external agents. |
| `/api/packages` | `GET` | List deployable agent packages known to Auspex. |
| `/api/packages/{id}` | `GET` | Read one deployable package definition. |
| `/api/packages/{id}/deploy` | `POST` | Create an `OmegonAgent` from package defaults plus overrides. |
| `/api/armory/packages` | `GET` | List Armory package records without installing anything. |
| `/api/armory/packages/{kind}/{id}` | `GET` | Read one Armory package by stable ref such as `profile/security-review`. |
| `/api/armory/plan` | `POST` | Produce a dry-run Armory install/deploy plan with policy gates. |
| `/api/armory/overlays` | `GET` | List Auspex deployment overlays persisted in the overlay ConfigMap. |
| `/api/armory/overlays` | `POST` | Create or replace one deployment overlay. |
| `/api/armory/overlays/{id}` | `GET` | Read one deployment overlay by Auspex-safe overlay id. |
| `/api/armory/overlays/{id}` | `PUT` | Create or replace one deployment overlay under a specific id. |
| `/api/armory/overlays/{id}` | `DELETE` | Remove one deployment overlay. |
| `/api/armory/preflight` | `POST` | Combine an Armory package, overlay, and deploy overrides into a non-mutating manifest preview. |
| `/api/agents` | `POST` | Create or server-side apply an `OmegonAgent`. |
| `/api/agents/{ns}/{name}` | `GET` | Read one managed agent with control-plane metadata. |
| `/api/agents/{ns}/{name}` | `PATCH` | Merge-patch an existing managed agent. |
| `/api/agents/{ns}/{name}/control-plane` | `GET` | Return ACP/WSS/health URLs and TLS posture. |
| `/api/agents/{ns}/{name}/acp` | `GET` | Authenticated WebSocket proxy to the selected managed agent's ACP stream. |
| `/api/agents/{ns}/{name}/rotate-control-tls` | `POST` | Bump TLS leaf/CA epochs and trigger reconciliation. |
| `/api/fleet/{ns}/{name}/sbom` | `GET` | Return SBOM status and artifact metadata. |
| `/api/audit` | `GET` | Return recent Kubernetes events involving managed agents. |
| `/api/secrets/grants` | `GET` | Return redacted secret-grant, identity, and control-TLS Secret projections. |

## Design Rules

- WebUI routes must enforce `AUSPEX_WATCH_NAMESPACE` when the operator is scoped.
- Secret endpoints must never return Secret data values. Returning key names,
  labels, annotations, and Secret type is acceptable.
- Control-plane metadata should include ACP/WSS URLs, TLS profile, CA epoch,
  leaf epoch, and validity posture so the UI can present rotation state.
- Mutations should operate through Kubernetes desired state. The WebUI should not
  bypass the operator by calling pod-local management endpoints directly.
- Managed agent manifests should set `terminalTool` explicitly. It defaults to
  `false` for headless Kubernetes pods; enabling Omegon's PTY-backed `terminal`
  tool requires `/dev/pts` and writable transcript/config storage.
- Operator permission flows should describe `/permissions` as canonical. Durable
  allow-always grants live under `profile.permissions.trustedDirectories`;
  `/trust` is compatibility-only.

## MVP Deploy Path

The deployed WebUI includes a `Deploy` workspace that acts as the first real
MVP test: Auspex must be able to create and observe agents inside the cluster
from its own control surface.

That workspace reads `/api/packages` and `/api/fleet`, then posts
`/api/packages/{id}/deploy` with package overrides:

```json
{
  "name": "home-media-operator",
  "namespace": "omegon-agents",
  "image": "ghcr.io/styrene-lab/omegon:0.26.5",
  "model": "anthropic:claude-sonnet-4-6",
  "secretName": "optional-env-secret",
  "authJsonSecret": "agent-auth-json",
  "connectors": ["aether", "discord"]
}
```

Security posture:

- `authJsonSecret` is preferred for provider auth material because it mounts one
  `auth.json` file at the runtime path instead of projecting a broad Secret
  through `envFrom`.
- `secretName` remains available for explicit environment-style tokens needed
  by extensions or integrations, but should be treated as higher blast radius.
- `connectors` maps to `spec.vox.connectors`; the reconciler is responsible for
  mounting extension material and enabling connector-specific runtime config.
- Package deploys enable identity provisioning and control-plane TLS by default,
  so the resulting daemon should expose `wss://` ACP and an operator-relative
  `acp_proxy_url`.

After a deploy, the same workspace should show the agent moving through the
cluster lifecycle:

1. `CR accepted` — the Kubernetes API has accepted the `OmegonAgent`.
2. `Workload created` — the operator has materialized a Deployment/CronJob/Job.
3. `Pod ready` — daemon Deployments report ready or available replicas.
4. `ACP reachable` — the operator has published an `acp_proxy_url`.

The WebUI preflight checks package requirements against the redacted
`/api/secrets/grants` projection. This does not prove the secret value is valid,
but it does prove that Auspex can see the required Kubernetes Secret envelope
without exposing secret contents. Ready daemon rows expose an `Open` action that
switches the browser onto the operator ACP proxy URL so Chat can operate against
the deployed agent path.

## Armory Planning Boundary

Armory-backed package support is split into read-only discovery, dry-run
planning, explicit Auspex deployment overlays, and policy-gated execution:

- `GET /api/armory/packages` fetches the configured Armory index
  (`AUSPEX_ARMORY_INDEX_URL`, default
  `https://armory.styrene.io/api/index.json`).
- `POST /api/armory/plan` accepts `{ "packageRef": "profile/security-review" }`
  and returns an `ArmoryInstallPlan`.
- `GET /api/armory/overlays` and `/api/armory/overlays/{id}` expose Auspex-owned
  deployment overlays from a Kubernetes ConfigMap. The store defaults to
  `auspex-armory-overlays` in `AUSPEX_WATCH_NAMESPACE`, or
  `AUSPEX_ARMORY_OVERLAYS_NAMESPACE`/`default` when the operator is not
  namespace-scoped. The ConfigMap name can be overridden with
  `AUSPEX_ARMORY_OVERLAYS_CONFIG_MAP`.
- `POST /api/armory/preflight` accepts
  `{ "packageRef": "profile/security-review", "overlayId": "security-review" }`
  or an inline `overlay`, then returns the install plan, generated
  `AgentPackage`, deploy request, Kubernetes `OmegonAgent` manifest preview,
  OCI supply-chain posture, policy gates, and namespace-scope errors without
  mutating the cluster.
- Preflight defaults to `ociPolicy: "warn"` and can be made strict with
  `{ "ociPolicy": "strict" }` or `AUSPEX_OCI_PREFLIGHT_POLICY=strict`. Strict
  mode requires the generated image to be digest-pinned, for example
  `ghcr.io/styrene-lab/omegon-agents@sha256:<digest>`, before the response is
  marked deployable.
- Planning may classify OCI artifacts, Omegon prompt/plugin payloads, native
  Omegon extensions, external integrations, Nex forge templates, required
  secrets, warnings, and policy gates.
- Planning never pulls artifacts, installs extensions, mutates Kubernetes, or
  grants secrets.
- `forge-template/*` packages are Nex-owned. Auspex may surface
  `canonicalFormat`, `minNex`, `destructiveCapabilities`, and
  `networkRequirements`, but Nex must validate or build them and Auspex policy
  must approve any destructive or networked execution.
- Armory `agent` and `profile` records require an explicit Auspex deployment
  overlay before becoming an `AgentPackage`; runtime fields such as image, mode,
  role, model, resources, control TLS profile, and secret grant bindings are not
  guessed from public Armory metadata.

Example overlay:

```json
{
  "id": "security-review",
  "armory": "profile/security-review",
  "mode": "daemon",
  "role": "security-reviewer",
  "image": "ghcr.io/styrene-lab/omegon:0.26.5",
  "model": "anthropic:claude-sonnet-4-6",
  "posture": "architect",
  "namespace": "omegon-agents",
  "required_secrets": ["agent-auth-json"],
  "control_tls_profile": "security-review",
  "mesh_role": "operator",
  "terminalTool": false
}
```

Preflight is the handoff point between browsing Armory and launching an agent.
It should be treated as the WebUI's review screen: operators can inspect
required secrets, destructive policy gates, Nex-owned build requirements, and
the exact manifest that would be applied before any deploy route is enabled.
Mutable tags are acceptable for local proving-ground work under warn mode, but
production package deploys should run strict OCI policy and reject tag-only
images until Nex/Armory provide signed digest-pinned packages with SBOM and
provenance referrers.

## Current Boundary

The first slice is intentionally thin: it gives the WebUI enough API to build
fleet, graph, deploy, identity, audit, and direct-line screens against real
cluster primitives. Control-plane metadata includes both the cluster-local
`acp_url` and an operator-relative `acp_proxy_url` for browser clients.

The ACP proxy dials upstream WSS with the selected agent's control-TLS Secret
when control-plane TLS is enabled. For mTLS agents, that Secret supplies the
client certificate, key, and CA bundle; missing or malformed material fails the
proxy closed instead of downgrading to plaintext.
