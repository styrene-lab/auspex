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
  "image": "ghcr.io/styrene-lab/omegon-agents:latest",
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

## Current Boundary

The first slice is intentionally thin: it gives the WebUI enough API to build
fleet, graph, deploy, identity, audit, and direct-line screens against real
cluster primitives. Control-plane metadata includes both the cluster-local
`acp_url` and an operator-relative `acp_proxy_url` for browser clients.

The ACP proxy dials upstream WSS with the selected agent's control-TLS Secret
when control-plane TLS is enabled. For mTLS agents, that Secret supplies the
client certificate, key, and CA bundle; missing or malformed material fails the
proxy closed instead of downgrading to plaintext.
