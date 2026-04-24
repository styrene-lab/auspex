# Brutus Deployment Status

**Date:** 2026-04-24
**Cluster:** brutus (K3s 1.33.6, 2 nodes: brutus control-plane + styrene-build worker)
**Namespace:** omegon-agents

## What's deployed

### auspex-operator
- **Status:** Running (1/1)
- **Image:** `docker.io/library/auspex-operator:latest` (built on Brutus via nix, imported into k3s containerd)
- **Service:** `auspex-operator.omegon-agents.svc:8080`
- **Fleet API:** `/api/fleet` returns `{"agents":[]}` (no OmegonAgent CRDs yet)
- **Health:** `/healthz` → `ok`
- **Scoped to:** `omegon-agents` namespace via `AUSPEX_WATCH_NAMESPACE`
- **Web UI:** Not baked into image yet (nix image doesn't include dist/)
- **RBAC:** ClusterRole with Deployments, Jobs, CronJobs, ConfigMaps, Services, Secrets, OmegonAgents

### CRDs installed
- `omegonagents.styrene.sh` (v1alpha1) — installed and working
- `externalagents.styrene.sh` — NOT installed (external controller errors in logs, non-blocking)

### Pre-existing workloads (hand-deployed, not operator-managed)
- `overnight-reviewer-omegon-agent` CronJob (hourly, running since 5d ago)
- `overnight-reviewer-final` Job (completed)
- Several errored pods from earlier CronJob runs

## What's NOT deployed yet

### Agent CRDs
No `OmegonAgent` CRDs exist. The operator is running but has nothing to reconcile.

### Web UI
The WASM bundle (`trunk build` produces 1.5MB optimized) is built locally but not in the deployed image. The nix-built image only contains the operator binary + cacert. To add the web UI, either:
- Rebuild the nix image expression to include the dist/ directory
- Or use the Dockerfile.operator multi-stage build (requires Docker on Brutus or CI)

### styrened mesh node
No styrened StatefulSet deployed. Needed before aether-connected agents can communicate.

### ExternalAgent CRD
Not installed — the `--crd` output format needs fixing (outputs JSON array, kubectl needs individual objects).

### StyreneIdentity provisioning
Operator root secret (`styrene-operator-identity`) not created in cluster. Identity provisioning will fail until it's seeded.

## Access

### SSH
```bash
ssh -i ~/.ssh/styrene-chris styrene@brutus
```
Key derived from StyreneIdentity root via `auspex-keygen ssh chris --export`.
Root at `~/.styrene/identity/root-secret` — deterministic re-derivation.

### kubectl
```bash
export KUBECONFIG=~/.kube/brutus.yaml
kubectl get pods -n omegon-agents
```

### Fleet API (from inside cluster)
```bash
curl http://auspex-operator.omegon-agents.svc:8080/api/fleet
curl http://auspex-operator.omegon-agents.svc:8080/healthz
```

### Fleet API (from local machine)
```bash
kubectl port-forward -n omegon-agents svc/auspex-operator 8080:8080
curl http://localhost:8080/api/fleet
```

## Cluster infrastructure available

| Component | Namespace | Notes |
|-----------|-----------|-------|
| ArgoCD | argocd | GitOps, could manage operator deployment |
| Vault | vault | Available for Tier 2 secret management |
| Vault Secrets Operator | vault-secrets-operator-system | Can sync Vault secrets to k8s |
| Envoy Gateway | envoy-gateway-system | TLS termination, could expose fleet API externally |
| cert-manager | cert-manager | TLS cert issuance |
| Argo Workflows | argo | CI/CD, already builds omegon |
| Argo Events | argo-events | GitHub webhook sensors for omegon, styrened, styrene-docs |
| Monitoring | monitoring | Prometheus stack for metrics |
| External Secrets | external-secrets | Alternative to Vault Secrets Operator |
| Velero | velero | Backup/restore |

## Build process

The operator binary is built directly on Brutus using nix:

```bash
ssh -i ~/.ssh/styrene-chris styrene@brutus
cd /tmp/auspex-build/auspex
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#gcc nixpkgs#pkg-config nixpkgs#openssl \
  --command cargo build --release -p auspex-operator
```

OCI image built with nix `dockerTools.buildLayeredImage` and imported via `k3s ctr images import`.

Source repos cloned at `/tmp/auspex-build/{auspex,omegon,styrene-rs}` on Brutus.

## Next steps

1. **Create first OmegonAgent CRD** — migrate overnight-reviewer from hand-deployed CronJob to CRD-managed
2. **Fix --crd output** — split into individual CRD documents for kubectl apply
3. **Install ExternalAgent CRD** — enables monitoring of off-cluster agents
4. **Bake web UI into image** — include dist/ in nix image for browser-based fleet management
5. **Deploy styrened** — mesh transport for aether-connected agents
6. **Seed operator identity** — create `styrene-operator-identity` Secret for StyreneID provisioning
7. **Expose fleet API** — HTTPRoute via Envoy Gateway at fleet.styrene.dev
8. **Wire into ArgoCD** — move from manual deployment to GitOps

## Session work summary (11 commits)

| Commit | Description |
|--------|-------------|
| `277855e` | nex agent profiles (7 image definitions) |
| `8b3f1a7` | Operator core: Job mode, ExternalAgent, identity, Vault, security hardening, CNCF conformance |
| `5001102` | Web bootstrap: ConnectHints, operator serves WASM |
| `4424194` | CI workflows: SBOM generation, cosign signing |
| `4056148` | Security tiers doc (Tier 1/2/3) |
| `5723e3f` | Omegon 0.16.0 tracking: posture system |
| `8222437` | Aether alignment: role, identity wiring, metrics |
| `cff06fc` | WASM compilation: all cfg-gating fixed |
| `128f512` | Trunk build config |
| `768de5e` | auspex-keygen: SSH key derivation from StyreneIdentity |
| (deployed) | Operator running on Brutus, fleet API live |
