# Auspex Agent Orchestration Plan

Auspex should manage Omegon instances as deployed agents, not as an embedded
single-agent chat shell. The primary agent is the operator-facing orchestrator;
its job is to plan, deploy, inspect, and steer a fleet of specialized Omegon
instances through secure control planes.

## Required Primitives

- Agent package: a buildable profile/image pair, eventually produced through
  `nex build-image`.
- Agent identity: a StyreneID-derived identity, mesh role, and rotation state.
- Control plane: an advertised `https://` and `wss://` endpoint with optional
  mTLS client authentication.
- Secret grant: a lease-bound grant that describes which references the agent
  may consume without exposing secret values to Auspex UI surfaces.
- Runtime placement: local process, Kubernetes workload, SSH-installed binary,
  or other managed target.
- Lifecycle state: deployment, health, session activity, grant consumption, and
  retirement status.

## Implementation Phases

1. Secure managed daemon contract.
   `OmegonAgent` declares control-plane TLS material; the operator mounts it,
   passes Omegon the TLS listener flags, and publishes secure fleet descriptors.

2. Deployment creation flow.
   Auspex creates `OmegonAgent` resources from profiles, requested posture,
   identity tier, secret grants, and placement constraints.

3. Secret delivery backends.
   Kubernetes uses projected/VSO-backed references by default. Nonstandard
   deployments use sealed bootstrap descriptors and one-time grant redemption.

4. Fleet topology UI.
   Graph becomes the deployed-agent topology: primary orchestrator, child
   agents, workflows, control links, identity posture, and secret grant state.

5. Orchestrator tool surface.
   The primary agent gets explicit tools for package build, deployment,
   retirement, grant issuance, topology inspection, and workflow assignment.

## UI Implications

- Chat is the direct line to the focused agent, with ACP configuration surfaced
  as controls rather than transcript clutter.
- Graph is fleet topology, not task authoring.
- Workflow owns task routing and handoff semantics.
- COP summarizes fleet health, policy violations, blocked deployments, and
  operator actions.

The first implemented slice is phase 1: `OmegonAgent.spec.controlPlane.tls`
drives TLS launch flags and secure fleet descriptors.
