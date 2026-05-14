---
id: librefang-peer-runtime-positioning
title: "LibreFang peer-runtime positioning for Auspex"
status: proposed
parent: auspex-multi-agent-runtime
tags:
  - peer-runtime
  - federation
  - librefang
dependencies:
  - auspex-multi-agent-runtime
  - auspex-runtime-backends
  - auspex-primary-coordinator
  - operator-security-tiers
related:
  - auspex-telemetry-aggregation-engine
  - auspex-live-canvas-widget-system
  - auspex-worker-inheritance
---

# LibreFang peer-runtime positioning for Auspex

## Source

Inspected local checkout:

- repo: `librefang/librefang`
- path: `/private/tmp/librefang-assess`
- commit: `7e2b1b6`

Key evidence:

- `README.md` positions LibreFang as an "Agent Operating System" for autonomous scheduled agents, Hands, dashboard, channels, provider routing, memory, MCP, A2A, and OpenAI-compatible API.
- `openapi.json` exposes `/api/agents`, `/api/agents/{id}/message`, `/api/agents/{id}/message/stream`, `/api/agents/{id}/sessions/{session_id}/stream`, `/api/hands`, `/api/workflows`, `/api/workflows/{id}/run`, `/api/mcp/*`, `/api/a2a/*`, `/api/metrics`, `/api/network/status`, `/v1/chat/completions`, and `/v1/models`.
- `crates/librefang-cli/src/mcp.rs` exposes running LibreFang agents as MCP tools named `librefang_agent_{name}`.
- `crates/librefang-cli/src/acp.rs` runs ACP over stdio and can proxy to a daemon-side socket.
- `crates/librefang-wire/src/lib.rs` documents OFP as authenticated and integrity-protected but plaintext, with confidentiality delegated to WireGuard/Tailscale/SSH tunnel/service-mesh mTLS.
- `crates/librefang-types/src/capability.rs` defines explicit capability grants and child-capability inheritance validation.
- `crates/librefang-runtime-mcp/src/lib.rs` implements outbound MCP taint scanning, per-tool/per-path exemptions, named rule sets, and block/warn/log actions.

## Positioning

LibreFang should not be treated as an Auspex dependency or a replacement for Omegon.
It is a peer runtime: another agent operating environment that Auspex can supervise, route to, observe, and govern.

This strengthens Auspex's product position.
Auspex should be the operator control plane that can manage a mixed fleet:

- Omegon workers for Styrene-native coding, ACP, workflows, and secure control-plane work.
- LibreFang daemon instances for autonomous Hands, broad channel adapters, OpenAI-compatible agent endpoints, and A2A/MCP federation experiments.
- Future third-party agent runtimes through the same peer-runtime adapter contract.

The important product claim is:

> Auspex is the operations layer above agent runtimes, not another agent runtime competing for every internal loop.

## Decision

### Add a peer-runtime adapter category

**Status:** proposed

Extend the current runtime backend model with a `peer-runtime` category.
A peer runtime is not spawned as an Omegon worker and is not assumed to speak the Omegon control-plane schema.
It is discovered or configured, then normalized into Auspex inventory.

Required adapter operations:

- `discover`
- `health`
- `list_agents`
- `inspect_agent`
- `send_message`
- `stream_session`
- `list_workflows`
- `run_workflow`
- `list_capabilities`
- `list_metrics`
- `list_policy_surface`

Optional operations:

- `install_package`
- `activate_package`
- `deactivate_package`
- `list_external_peers`
- `send_federated_task`

### Treat LibreFang as the first external peer-runtime adapter

**Status:** proposed

The LibreFang adapter should use network/API boundaries:

- REST for inventory and actions: `/api/agents`, `/api/hands`, `/api/workflows`, `/api/mcp/*`, `/api/a2a/*`.
- SSE for live agent/session output: `/api/agents/{id}/message/stream` and `/api/agents/{id}/sessions/{session_id}/stream`.
- Prometheus text metrics: `/api/metrics`.
- OpenAI-compatible provider path: `/v1/models` and `/v1/chat/completions`.
- MCP stdio bridge: `librefang mcp`, if Auspex/Omegon wants to call LibreFang agents as tools.
- ACP stdio bridge: `librefang acp`, useful for editor-like single-agent surfaces but not the first Auspex control plane.

Do not use OFP as the default Auspex transport.
OFP can be represented as a federated peer-link only when deployed behind an approved encrypted overlay or service-mesh mTLS.

### Normalize LibreFang Hands as packages, not Auspex tasks

**Status:** proposed

LibreFang Hands map best to Auspex `ArmoryPackage` / `CapabilityPackage` concepts, not to Flynt tasks or `omegon-flow` nodes directly.

Recommended mapping:

- LibreFang Hand definition -> `ExternalCapabilityPackage`
- Active Hand instance -> `ExternalAgentInstance`
- Hand requirements/readiness -> `PolicyReadiness`
- Hand activation/deactivation -> `DeploymentIntent`
- Hand schedule/workflow metadata -> `WorkSource`

Auspex should show a Hand as deployable capability owned by LibreFang, while its resulting active agent appears in Graph and Fleet inventory.

### Borrow the policy ideas, not the crate graph

**Status:** proposed

The most valuable implementation ideas to copy into Auspex/Omegon are:

- per-agent immutable capabilities
- child capability inheritance validation
- per-tool/per-path MCP taint policy
- taint rule sets with `block`, `warn`, and `log`
- idempotency keys on agent spawn and task dispatch
- daemon metrics shaped for Prometheus scraping
- explicit approval queue APIs
- active session stream attach

These should be reimplemented or adapted to Styrene identity, policy, and audit primitives.
Do not import the LibreFang Rust crates into Auspex or Omegon core.

## Why this helps Auspex

### Graph

Graph should become runtime-agnostic topology.
LibreFang gives a concrete second runtime that forces the model to stop assuming "agent == Omegon process."

Graph node kinds should include:

- `OmegonInstance`
- `LibreFangDaemon`
- `LibreFangAgent`
- `LibreFangHand`
- `McpServer`
- `A2APeer`
- `WorkflowRun`
- `PolicyApproval`

Edges should include:

- `supervises`
- `hosts`
- `message_route`
- `workflow_run`
- `uses_mcp`
- `federates_a2a`
- `requires_policy`
- `emits_metrics`

### Chat / Direct Line

Chat remains the direct line to the selected coordinator or selected agent.
With LibreFang attached, the selected target may be:

- Primary Coordinator backed by Omegon.
- A LibreFang agent through `/api/agents/{id}/message`.
- A LibreFang session stream through SSE.
- A LibreFang agent exposed as an MCP tool to Omegon.

The UI should make the runtime boundary visible:

- target runtime
- target agent
- transport
- auth posture
- policy posture
- streaming availability

### COP

COP should summarize peer-runtime health and current work:

- LibreFang daemon uptime, active agents, active sessions, costs, restarts, and tool calls from `/api/metrics`.
- active Hands and degraded requirements from `/api/hands/active`.
- pending approvals from `/api/approvals`.
- A2A trusted/pending external agents from `/api/a2a/agents`.
- MCP server health from `/api/mcp/health`.

### Workflow

LibreFang workflows should not replace `omegon-flow`.
They should appear as external workflow definitions/runs under a peer runtime.

Auspex can:

- list them
- run them
- inspect runs
- relate them to `ExecutionLane`
- create adapter nodes that call LibreFang workflow runs

Auspex should not try to round-trip arbitrary LibreFang workflow definitions into `omegon-flow` until there is a proven mapping.

### Primary Coordinator

The Primary Coordinator should understand LibreFang as an external runtime option:

- use LibreFang when the work is channel-heavy, scheduled, Hand-shaped, or already packaged there
- use Omegon when the work is Styrene-native, code/workspace-bound, identity-sensitive, or requires WSS/mTLS control-plane guarantees
- use A2A/MCP only through explicit policy and audit gates

## Integration phases

### Phase 0: documentation and adapter contract

- Add `PeerRuntimeDescriptor`.
- Add `PeerAgentDescriptor`.
- Add `PeerRuntimeAdapter` trait/design doc.
- Add LibreFang adapter configuration shape:

```toml
[[peer_runtimes]]
id = "local-librefang"
kind = "librefang"
base_url = "http://127.0.0.1:4545"
auth = { mode = "api-key-ref", ref = "librefang/local" }
transport_security = "loopback-dev"
```

### Phase 1: read-only LibreFang inventory

- Probe `/v1/models` and `/api/agents`.
- Probe `/api/hands` and `/api/hands/active`.
- Scrape `/api/metrics`.
- Render LibreFang daemon + agents in Graph.
- Add COP rollups for active agents, active sessions, pending approvals, and MCP health.

### Phase 2: direct-line attach

- Add selected-agent chat target for LibreFang agents.
- Use `/api/agents/{id}/message` for request/response.
- Use `/api/agents/{id}/message/stream` or session stream for live output.
- Preserve target runtime in audit records.

### Phase 3: controlled actuation

- Add policy-gated actions:
  - activate/deactivate Hand
  - run LibreFang workflow
  - send A2A task
  - install MCP template/server
- Require idempotency keys for spawn/task/run calls where supported.
- Record all actions as `DeploymentIntent` or `CoordinationEvent`.

### Phase 4: federation experiments

- Treat A2A as the preferred cross-agent federation protocol candidate.
- Treat OFP as LibreFang-internal or overlay-only.
- Add trust-state UI for pending/trusted A2A agents.
- Require Styrene identity or approved external identity binding before cross-network production use.

## Security posture

LibreFang's REST API and OpenAI-compatible API are easiest to integrate but must be treated as privileged control surfaces.

Rules:

- Loopback HTTP is acceptable only for local-dev peer attachment.
- Remote LibreFang must be behind TLS, mTLS, or a trusted ingress controlled by Auspex policy.
- OFP must not cross untrusted networks without an encrypted overlay.
- A2A task send must require a policy gate and audit correlation id.
- API keys should be referenced through Auspex secret refs, not copied into config.
- LibreFang capability grants should be imported as advisory metadata until Auspex can verify enforcement.
- MCP taint policy should be mirrored into Auspex's own policy model rather than trusted blindly from the peer.

## Product risks

- LibreFang has substantial product overlap with Auspex dashboards, workflows, memory, telemetry, and channel adapters.
- Treating LibreFang as a UI to embed would blur Auspex's product boundary.
- Treating LibreFang as a crate dependency would import a large dependency and protocol surface.
- Treating LibreFang as only an OpenAI-compatible provider would underuse its strengths.

The right middle path is peer-runtime supervision.

## Recommended next step

Implement read-only peer runtime discovery for local LibreFang:

1. Add a config entry for peer runtimes.
2. Probe `GET /v1/models`, `GET /api/agents`, `GET /api/hands/active`, and `GET /api/metrics`.
3. Normalize results into the existing fleet/deployment telemetry layer.
4. Render LibreFang daemon and agents in Graph as external runtime topology.
5. Add COP rollups only after the normalized telemetry path exists.

This gives Auspex immediate leverage from LibreFang's existing work without importing its kernel or duplicating its dashboard.
