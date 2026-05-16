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

| Route | Method | Purpose |
|-------|--------|---------|
| `/api/fleet` | `GET` | List managed and external agents. |
| `/api/agents` | `POST` | Create or server-side apply an `OmegonAgent`. |
| `/api/agents/{ns}/{name}` | `GET` | Read one managed agent with control-plane metadata. |
| `/api/agents/{ns}/{name}` | `PATCH` | Merge-patch an existing managed agent. |
| `/api/agents/{ns}/{name}/control-plane` | `GET` | Return ACP/WSS/health URLs and TLS posture. |
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

## Current Boundary

The first slice is intentionally thin: it gives the WebUI enough API to build
fleet, graph, deploy, identity, audit, and direct-line screens against real
cluster primitives. ACP stream proxying is not implemented yet; the current API
returns the selected agent's `acp_url` and transport posture so the next slice
can add an authenticated `/api/agents/{ns}/{name}/acp` proxy.
