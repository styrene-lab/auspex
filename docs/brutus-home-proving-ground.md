# Brutus Home Proving Ground

Brutus should become the first real home-system proving ground for Auspex-managed
long-running Omegon agents. The goal is not to make one general agent watch the
house. The goal is to prove that Auspex can supervise several focused daemon
agents with distinct profiles, identities, secrets, and control planes.

## Agent Set

| Agent | Scope | Why it exists |
|---|---|---|
| `home-media-operator` | `media` namespace and media service APIs | Operates Jellyfin, Jellyseerr, Radarr, Sonarr, Prowlarr, qBittorrent, SABnzbd, and related media workloads. |
| `home-infra-sentinel` | Brutus platform health | Watches nodes, cert-manager, Envoy Gateway, Vault, Keycloak, ArgoCD, backups, and resource pressure. |
| `home-forge-steward` | Styrene forge and release lane | Watches Forgejo, ArgoCD sync, image builds, package publication, and release readiness. |
| `home-knowledge-curator` | Runbooks and operational memory | Maintains operational notes, incident summaries, board hygiene, and handoff context. |

Each agent is declared as a daemon `OmegonAgent` in
`deploy/brutus-home-proving-ground/`. Each profile lives under `profiles/` so
`nex build-image` can produce specific images once the agent package lane is
ready.

Auspex also exposes these package definitions through the operator API:

| Route | Purpose |
|---|---|
| `/api/packages` | List deployable built-in packages. |
| `/api/packages/{id}` | Read one package definition. |
| `/api/packages/{id}/deploy` | Create an `OmegonAgent` from package defaults plus optional overrides. |

This is intentionally a local built-in catalog first. The same shape is the
bridge to Armory/Signum discovery once package publication is the source of
truth.

## Current Deployment Shape

- Namespace: `omegon-agents`
- Runtime: `OmegonAgent` daemon mode
- Provider auth reference: narrow `authJsonSecret` projection per agent, or
  Vault/VSO/CSI materialization of the same provider `auth.json` grant.
- Identity: enabled through `spec.identity.provision`
- Control plane: WSS/HTTPS TLS enabled through `spec.controlPlane.tls`
- Image: temporarily `ghcr.io/styrene-lab/omegon:0.26.5`

The image is intentionally still the shared fallback. Once the profile build lane
is stable, pin each resource to the profile-specific image digest built from:

```bash
nex build-image profiles/home-media-operator.toml --tag <version>
nex build-image profiles/home-infra-sentinel.toml --tag <version>
nex build-image profiles/home-forge-steward.toml --tag <version>
nex build-image profiles/home-knowledge-curator.toml --tag <version>
```

## Apply

Apply only after the Brutus `OmegonAgent` CRD has been updated from the current
Auspex repo. The live cluster CRD from 2026-04-24 rejects newer fields such as
`spec.controlPlane`, `spec.posture`, and camelCase secret references.

Validate first:

```bash
kubectl --kubeconfig ~/.kube/brutus.yaml apply \
  --server-side --dry-run=server \
  -k deploy/brutus-home-proving-ground
```

Apply:

```bash
kubectl --kubeconfig ~/.kube/brutus.yaml apply \
  --server-side \
  -k deploy/brutus-home-proving-ground
```

## Required Before Live Use

1. Redeploy `auspex-operator` from the current repo state.
2. Seed `styrene-operator-identity` or switch these agents to a Vault-backed
   identity tier.
3. Ensure each provider credential grant is projected as a narrow
   `authJsonSecret` or Vault/VSO/CSI file projection. Avoid placing every
   provider token in one broad `omegon-agent-secrets` env Secret.
4. Build and pin profile-specific images.
5. Add API-specific secret grants for the media stack rather than putting every
   home credential into one broad Secret.
6. Expose Auspex WebUI/API behind Envoy only after auth is enforced.

## Why This Mix

These agents test the pieces Auspex must eventually manage in production:

- multiple long-running daemons;
- distinct operational scopes;
- CRD-managed lifecycle instead of hand-built CronJobs;
- profile-specific package intent;
- per-agent identity and control TLS;
- secret references without raw value handling;
- future Graph view topology with a primary coordinator supervising useful
  detached services.
