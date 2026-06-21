+++
title = "OCI Container Deployment — Landscape Research"
tags = ["research","oci","deployment","containers","homelab"]
+++

# OCI Container Deployment — Landscape Research

# OCI Container Deployment — Landscape Research

**Status:** Research / Seed  
**Date:** 2026-06-21  
**Context:** Auspex needs a deployment path for agents as OCI containers on homelab targets (Docker, Podman, bare VMs), distinct from the k8s operator path already in development. If Auspex itself is containerized, socket access and privilege constraints apply.

---

## Current State

The existing code establishes the skeleton but doesn't have an execution path yet:

| File | What exists |
|------|-------------|
| `runtime_types.rs` | `BackendKind::OciContainer` variant is declared |
| `container_discovery.rs` | Shells out to `podman ps` + `curl` for passive discovery |
| `agent_packages.rs` | Generates `OmegonAgent` k8s CRD manifests; no OCI-native deploy logic |
| `auspex-operator` | Uses `kube-rs`; handles k8s only |

Nothing currently creates, starts, or stops OCI containers natively — all container interaction is read-only CLI shelling. The `OciContainer` backend path needs an actual driver.

---

## Deployment Target Landscape

### Tier 1 — Homelab Docker (most common)

Docker daemon exposing a Unix socket at `/var/run/docker.sock`. The vast majority of homelabbers are here: Unraid, TrueNAS, Proxmox VMs, bare Ubuntu/Debian with Docker installed. This is the first target.

API surface: Docker Engine REST API (HTTP over Unix socket). Fully documented, very stable.

### Tier 2 — Homelab Podman

Growing adoption, especially on RHEL/Fedora derivatives and privacy-conscious homelabbers. Key differences from Docker:

- **Daemonless**: no persistent daemon; each `podman` invocation is a process. *But* Podman exposes a REST API via a systemd socket unit — effectively the same HTTP-over-socket model as Docker.
- **Rootless by default**: socket at `/run/user/{uid}/podman/podman.sock`. No root required, no host network namespace by default.
- **Docker-compatible API**: Podman's REST API is a superset of the Docker Engine API. The same HTTP calls work on both.

Socket paths:
```
Rootful Podman:   /run/podman/podman.sock
Rootless Podman:  /run/user/<uid>/podman/podman.sock  (or $XDG_RUNTIME_DIR/podman/podman.sock)
Docker:           /var/run/docker.sock
```

### Tier 3 — Remote / VM targets

Docker/Podman over TCP (`DOCKER_HOST=tcp://192.168.x.x:2375`) or SSH tunnel (`DOCKER_HOST=ssh://user@host`). Less common in homelab but important for deploy-to-NAS patterns where Auspex runs on a different machine than the agent container.

Podman also supports `podman-remote` which is SSH-tunneled by default (safer than raw TCP).

### Tier 4 — k8s (already covered)

k3s, RKE2, kind, kubeadm — handled by the `auspex-operator` via `kube-rs`. Out of scope for this work item except for interface consistency.

---

## Rust Ecosystem: What to Use

### `bollard` — the primary driver

**Crate:** `bollard` (crates.io)  
**Repo:** https://github.com/fussybeaver/bollard  
**Maturity:** Production-grade. Async (Tokio/Hyper), actively maintained, used in production tooling.

Bollard is the only production-quality async Rust client for both Docker and Podman APIs. It connects via:
- Unix socket (default, most common)
- Named pipe (Windows Docker Desktop)
- HTTP (TCP daemon)
- SSL/TLS

```rust
// Docker via default socket
let docker = Docker::connect_with_local_defaults()?;

// Podman rootless
let docker = Docker::connect_with_unix(
    "/run/user/1000/podman/podman.sock",
    120,
    API_DEFAULT_VERSION,
)?;

// Remote via TCP
let docker = Docker::connect_with_http("http://192.168.1.10:2375", 120, API_DEFAULT_VERSION)?;
```

Key operations available:
- `create_container` / `start_container` / `stop_container` / `remove_container`
- `pull_image` (streaming)
- `list_containers` (replaces the current `podman ps` shell-out)
- `inspect_container`
- `logs` (streaming)
- `exec_in_container`
- Container events stream (for lifecycle tracking without polling)

Podman compatibility: Bollard's README explicitly lists Podman as a first-class supported runtime. The same `Docker` struct works against Podman's socket.

**Verdict: This is the right choice for Tier 1 and Tier 2 deployments. Replace the `podman ps` shell-out in `container_discovery.rs` with a bollard client.**

### `oci-client` — registry pull/push

**Crate:** `oci-client` (crates.io, maintained under `oras-project/rust-oci-client`)  
**What it does:** Implements the OCI Distribution Specification — pull/push images from any OCI-compliant registry (Docker Hub, GHCR, Quay, self-hosted, etc.) *without* requiring a running Docker/Podman daemon.

Relevant if Auspex needs to:
- Pre-pull an agent image to verify it exists/is accessible before deploy
- Pull an image directly to the host when no daemon is running (bootstrap scenario)
- Inspect image manifests (digest pinning, platform check)

Not relevant for the core "spawn a container from an already-accessible image" path — bollard handles that with image pull built in.

### `oci-spec` — type definitions

**Crate:** `oci-spec`  
Provides typed structs for OCI image config, runtime config, and distribution spec. Useful if we need to parse or construct OCI manifests (e.g., when generating container run specs programmatically).

### `containerd-client` — skip for now

**Crate:** `containerd-client`  
gRPC client for the containerd API. Only relevant if targeting the containerd socket directly (i.e., bypassing the Docker/Podman daemon and talking to containerd on a k8s node). This is lower-level than needed for homelab OCI deploy. Revisit if we need node-level container management on k8s clusters.

### `kube-rs` — already present in operator

Already used in `auspex-operator`. Not needed in the homelab OCI path.

### `firecracker` SDK — future consideration

Firecracker is a microVM monitor (not a container runtime). Its Rust SDK (`firecracker` crate) lets you create and manage microVMs via its REST API. Relevant for high-isolation agent sandboxing (each agent in its own VM), not for typical homelab OCI deploy. File for the security/isolation track.

---

## Socket Access When Auspex Is Containerized

This is the hard problem. If Auspex runs as a container itself, it cannot access the host's container runtime without explicit permission. Options, roughly in order of preference:

### Option A: Socket bind-mount (user opt-in)

Mount the host Docker or Podman socket into the Auspex container:

```yaml
# docker-compose.yml for Auspex
volumes:
  - /var/run/docker.sock:/var/run/docker.sock
  # or Podman rootless:
  - /run/user/1000/podman/podman.sock:/run/user/1000/podman/podman.sock
```

Auspex then connects with bollard via the mounted socket path. This is the **DinD (Docker-in-Docker) socket pattern** — not true DinD, no nested daemon, just shared socket access.

**Risk:** The container with Docker socket access is effectively root on the host. Any code in Auspex that misbehaves can trivially escape the container via the socket. OWASP explicitly recommends against it (Rule #1 of their Docker Security Cheat Sheet).

**Mitigation that makes it acceptable:** Run a **Docker socket proxy** as a sidecar (e.g., `tecnativa/docker-socket-proxy` or equivalent). The proxy exposes only the specific API endpoints Auspex needs (create, start, stop, list, logs) and blocks dangerous operations (exec with root, privileged containers, host network access). Auspex connects to the proxy, not the raw socket.

```yaml
services:
  docker-proxy:
    image: tecnativa/docker-socket-proxy:latest
    environment:
      CONTAINERS: 1
      IMAGES: 1
      NETWORKS: 1
      VOLUMES: 0
      EXEC: 0
      AUTH: 0
      SECRETS: 0
      POST: 1   # allow container create/start
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
    networks:
      - internal

  auspex:
    environment:
      DOCKER_HOST: tcp://docker-proxy:2375
    networks:
      - internal
```

This is the standard pattern used by Traefik, Portainer, Dozzle, and similar homelab tools. Well-understood, widely deployed.

### Option B: DOCKER_HOST env var pointing to TCP daemon

User enables the Docker daemon's TCP listener (or SSH tunnel) and sets `DOCKER_HOST`. Bollard supports this natively.

More explicit configuration burden on the user, but no socket mount required. Appropriate for "deploy to a remote host" use cases (Auspex on laptop, agent containers on NAS).

### Option C: Podman rootless socket (lower privilege)

Podman's rootless socket doesn't require root on the host. A rootless Podman socket gives you only the ability to create containers running as the user who owns the socket. A compromised Auspex container can only affect that user's containers, not the full host.

This is significantly safer than the Docker socket pattern. For homelabbers who have Podman, recommend this path explicitly.

### Option D: External agent runner (sidecar or companion binary)

Don't do container management from inside Auspex at all. Instead, have a small companion process (`auspex-runner` or similar) on the host that listens on a local socket or HTTP endpoint, and has the Docker socket access. Auspex sends deployment intents to this companion, which executes them.

This is architecturally cleaner (separation of privilege) but significantly more complex to install and operate for homelabbers. Viable for an enterprise/advanced tier later. Not the right first move.

---

## Proposed Architecture: `OciBackend` Driver

Based on the research, the right approach is:

```
auspex-core/
  src/
    oci_backend.rs          ← new: trait + DockerBackend / PodmanBackend impls
    container_discovery.rs  ← refactor: replace podman-ps shell-out with bollard
```

### Backend trait

```rust
#[async_trait]
pub trait OciBackend: Send + Sync {
    /// Pull an image if not already present. Streams progress.
    async fn pull_image(&self, image: &str) -> Result<()>;

    /// Create and start an agent container. Returns container ID.
    async fn launch(&self, spec: &OciLaunchSpec) -> Result<String>;

    /// Stop and remove an agent container.
    async fn terminate(&self, container_id: &str) -> Result<()>;

    /// List running agent containers (filtered by label).
    async fn list_agents(&self) -> Result<Vec<DiscoveredContainer>>;

    /// Stream logs from a container (async iterator).
    async fn logs(&self, container_id: &str) -> Result<impl Stream<Item = LogLine>>;
}
```

### Launch spec

```rust
pub struct OciLaunchSpec {
    pub image: String,
    pub name: String,
    pub host_port: u16,           // mapped to container port 7842
    pub env: Vec<(String, String)>,
    pub labels: BTreeMap<String, String>,
    pub resource_limits: Option<ResourceLimits>,
    pub pull_policy: PullPolicy,  // Always | IfNotPresent | Never
}
```

### Connection negotiation

At startup, Auspex probes for available backends in priority order:
1. `DOCKER_HOST` env var (explicit user configuration, any runtime)
2. `/var/run/docker.sock` (Docker, rootful)
3. `$XDG_RUNTIME_DIR/podman/podman.sock` (Podman rootless)
4. `/run/podman/podman.sock` (Podman rootful)

First socket that responds to a `ping` wins. Emit a log line indicating which runtime was found and which socket path is in use.

---

## Container Labels for Discovery

Replace the image-name-based filter (`image.contains("auspex-agents")`) with a label filter. More robust, allows any image name:

```
styrene.sh/managed-by=auspex
styrene.sh/agent-id=<instance_id>
styrene.sh/agent-package=<package_id>
auspex.styrene.sh/host-port=7845
```

Bollard can filter `list_containers` by label directly without parsing image names.

---

## Security Posture for Spawned Agent Containers

When Auspex launches agent containers, apply these constraints by default:

| Constraint | Why |
|---|---|
| No `--privileged` | Agents don't need kernel capabilities |
| Drop all capabilities, add only what's needed | Defence in depth |
| Read-only root filesystem | Agent state goes in mounted volume |
| No host network (`--network=bridge` or named network) | Prevent host network access |
| Memory + CPU limits from `PackageResources` | Prevent resource starvation |
| Non-root UID in container | Standard practice |
| `--security-opt no-new-privileges` | Prevent setuid escalation |

These map directly to bollard's `CreateContainerOptions` → `HostConfig`.

---

## Gaps and Open Questions

1. **Image pull authentication**: Pulling from GHCR requires credentials. Where do homelab users store registry credentials? Options: Docker credential store (OS keychain), `~/.docker/config.json`, or a secret passed as env var. Bollard supports passing auth via `CreateImageOptions`.

2. **Port allocation**: The current model maps container port 7842 to a host port. How does Auspex pick the host port when launching (not just discovering)? Needs a port-allocation strategy (random in range, or explicit in launch spec).

3. **Networking between Auspex and agents**: If Auspex is containerized and agents are also containers, they need to be on the same Docker network for Auspex to reach agents at `127.0.0.1:<port>`. Alternatively, use the container's bridge IP. Both work; explicit shared network is cleaner.

4. **Lifecycle persistence across Auspex restarts**: If Auspex restarts, containers keep running. The `list_agents` + label filter re-discovers them. But if Auspex crashed mid-launch, a container might be in a partial state. Need a reconciliation loop (not just discovery at startup).

5. **`auspex-core` vs `auspex-operator` split**: Where does the OCI backend driver live? `auspex-core` is shared between desktop and web builds; bollard is a native-only dependency. It should be behind `#[cfg(not(target_arch = "wasm32"))]` or a `desktop` feature flag, consistent with how `container_discovery.rs` is gated today.

6. **Podman vs Docker API divergence**: The Podman REST API is largely Docker-compatible but has some divergence (pod management, quadlet, etc.). Bollard abstracts most of this, but we should test against both and document known differences (e.g., Podman's auto-update labels, quadlet-based lifecycle).

7. **Socket proxy as a first-class concern**: Should Auspex ship a compose template that includes the socket proxy sidecar? Probably yes — it's the safe default for containerized Auspex.

---

## Recommended Next Steps

1. **Add bollard dependency** to `auspex-core/Cargo.toml` behind a `desktop` feature flag.
2. **Implement `OciBackend` trait** with a `BollardBackend` that handles Docker + Podman via runtime detection.
3. **Refactor `container_discovery.rs`** to use bollard's `list_containers` instead of `podman ps` shell-out. Keep the existing `podman ps` path as a fallback if bollard socket connect fails.
4. **Implement `launch` and `terminate`** in `BollardBackend` using `create_container` + `start_container`.
5. **Design node**: Create a design node to track the OCI deployment driver decisions before implementing the full feature.
6. **Socket proxy compose template**: Add an `examples/compose/auspex-with-socket-proxy.yml` to document the safe containerized-Auspex pattern.

---

## Crate Versions (as of research date)

| Crate | Version | Notes |
|---|---|---|
| `bollard` | `0.18.x` | Latest stable; async Docker/Podman API |
| `oci-client` | `0.14.x` | oras-project; OCI Distribution spec |
| `oci-spec` | `0.7.x` | OCI image/runtime spec types |
| `containerd-client` | `0.6.x` | gRPC containerd; skip for now |
| `kube` | `0.99.x` | Already in operator |
