# Brutus Control-Plane MVP

This overlay deploys the Auspex operator plus deployed WebUI into
`omegon-agents`. It assumes the CRDs have already been applied:

```bash
cargo run -p auspex-operator -- --crd | kubectl apply -f -
```

Seed the required Secrets before applying the overlay:

```bash
kubectl -n omegon-agents create secret generic auspex-operator-api-token \
  --from-literal=token="$(openssl rand -hex 32)"

dd if=/dev/urandom bs=32 count=1 2>/dev/null | \
  kubectl -n omegon-agents create secret generic styrene-operator-identity \
    --from-file=root-secret=/dev/stdin \
    -l styrene.sh/identity=operator

kubectl -n omegon-agents create secret generic auspex-primary-openai-codex-auth \
  --from-file=auth.json=/path/to/auth.json
```

Apply:

```bash
kubectl apply -k deploy/brutus-control-plane
```

Smoke check:

```bash
kubectl -n omegon-agents rollout status deploy/auspex-operator
kubectl -n omegon-agents port-forward svc/auspex-operator 8080:8080
curl -H "Authorization: Bearer $AUSPEX_API_TOKEN" http://localhost:8080/api/fleet
```

For browser direct-line use, connect to each managed agent through the
operator-relative `control_plane.acp_proxy_url`. Do not expose pod-local
`*.svc` ACP URLs outside the cluster.
