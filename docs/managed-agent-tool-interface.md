---
id: managed-agent-tool-interface
title: "Managed agents as supervised tools of the primary"
status: exploring
parent: auspex-session-dispatcher
tags: [multi-agent, dispatcher, tool-interface, ui, supervision]
open_questions:
  - "What exact mutation tools should follow the V1 read-only agents_status bridge, and which require operator approval?"
  - "What normalized payload schema and retention policy should each managed-agent event kind use, including ordering/replay behavior after reconnect?"
  - "How should existing delegate and cleave runtime events/results adapt into the common ManagedAgentRun model without losing provider-specific evidence?"
  - "[assumption] Existing worker profiles contain sufficient capability/scope metadata for Auspex to determine eligibility before delegation."
dependencies: []
related:
  - auspex-multi-agent-runtime
  - auspex-worker-profiles
  - auspex-worker-inheritance
  - auspex-instance-registry-schema
  - auspex-detached-service-lifecycle
---

# Managed agents as supervised tools of the primary

## Overview

Define attached and Auspex-managed Omegon instances as stateful, supervised tool endpoints available to the session primary/dispatcher — not as alternate chat participants.

Operator ↔ Primary/Dispatcher ↔ typed managed-agent tools ↔ Auspex Supervisor ↔ authenticated worker control planes.

The primary chooses when and why to delegate. Auspex owns worker selection/launch eligibility, policy enforcement, transport, budgets, event normalization, provenance, cancellation, and result harvesting. Managed worker output is projected as structured tool-call activity in the primary transcript; raw worker transcript remains a diagnostic surface.

This node unifies attached runtimes, delegates, cleave workers, and background managed agents behind one run-oriented contract with an event stream underneath.

## Decisions

### Run-oriented public contract with event stream underneath

**Status:** accepted

**Rationale:** 

### Auspex owns the tool boundary over worker runtimes

**Status:** accepted

**Rationale:** 

### Separate run identity from worker identity

**Status:** accepted

**Rationale:** 

### Typed normalized event stream, not transcript passthrough

**Status:** accepted

**Rationale:** 

### One conversation; agent work appears as tool-call cards

**Status:** accepted

**Rationale:** 

### Five-level UI hierarchy separates conversation from supervision

**Status:** accepted

**Rationale:** 

### Operator intervention is explicit and observable by primary

**Status:** accepted

**Rationale:** 

### V1 bounded non-recursive delegation policy

**Status:** accepted

**Rationale:** 

### Bundled native extension is the internal primary bridge

**Status:** accepted

**Rationale:** The Auspex-owned Omegon primary runs in a separate process, so its model-visible tools cannot hold an in-process reference to `AppController`. Auspex therefore bundles a private native extension, preloads it only into the primary it launches, and uses the generic Omegon Extension SDK as the process boundary. The extension is an internal application component: it is not an Armory package, public plugin, independently versioned product, or reusable source of managed-agent semantics. Auspex owns its manifest, binary, protocol version, installation, enablement, and compatibility checks.

### Auspex remains the authority behind the bridge

**Status:** accepted

**Rationale:** The extension advertises Auspex-owned tools to Omegon but does not own supervision state, policy, worker transport, or projection semantics. It forwards typed requests over a private authenticated local bridge to the canonical `ManagedAgentSupervisorRuntime`. Omegon remains the singular primary agent runtime and generic extension host; it gains no Auspex-specific built-in feature.

### Primary identity is launch-bound, never model-asserted

**Status:** accepted

**Rationale:** Model-visible tool arguments must not contain `parent_session_id` as an authorization claim. Auspex provisions the bundled extension with a short-lived capability bound to the owned primary/session. The bridge derives parent scope from that capability and accepts only subordinate selectors such as an optional `run_id`. Cross-parent access is impossible at the protocol boundary.

### Internal bridge fails closed

**Status:** accepted

**Rationale:** Auspex explicitly enables the bundled extension when launching its primary and verifies the loaded extension generation before reusing an existing process. Missing extension binary, incompatible bridge protocol, failed authentication, or absent supervisor endpoint makes managed-agent tools unavailable and visible as degraded startup state; the extension must never return mock or stale-success data.

## Open Questions

- What exact mutation tools should follow the V1 read-only `agents_status` bridge, and which require operator approval?
- What normalized payload schema and retention policy should each managed-agent event kind use, including ordering/replay behavior after reconnect?
- How should existing delegate and cleave runtime events/results adapt into the common ManagedAgentRun model without losing provider-specific evidence?
- [assumption] Existing worker profiles contain sufficient capability/scope metadata for Auspex to determine eligibility before delegation.

## Internal Bridge Contract

The production integration is a bundled native extension named `auspex-managed-agents`. Its lifecycle is inseparable from the Auspex application release even though it is a separate executable for process isolation.

```text
Auspex AppController
  -> canonical ManagedAgentRunProjection
  -> private authenticated local bridge
  -> bundled auspex-managed-agents extension
  -> generic Omegon Extension SDK over stdio
  -> Auspex-owned Omegon primary
```

### Ownership

- **Auspex app:** supervision authority, policy, projection, bridge listener, capability issuance, launch/restart decisions.
- **Bundled extension:** protocol adapter and tool-definition provider only.
- **Omegon core:** generic primary agent loop, extension discovery, tool registry, and JSON-RPC routing only.
- **Worker runtimes:** subordinate execution endpoints; never directly reachable from the primary tool implementation.

### Distribution and discovery

- Build the extension as an Auspex workspace artifact.
- Package it beside or inside the Auspex application bundle.
- Materialize its manifest and executable into an Auspex-controlled Omegon home at launch.
- Add `auspex-managed-agents` to the owned child's explicit enabled-extension set while preserving unrelated configured extensions.
- Do not publish it to Armory, discover it from a user-global extension catalog, or treat independent extension installation as supported.
- Refuse to reuse an existing primary unless startup metadata proves the expected extension and bridge protocol generation are loaded.

### Authentication and transport

- Use a private local IPC endpoint with restrictive filesystem permissions; no public listener is required.
- Generate a short-lived random capability for each owned-primary launch.
- Pass endpoint discovery as non-secret bootstrap configuration and capability material through the extension secret/bootstrap channel or an inherited secret descriptor; never put the token in model-visible arguments, logs, process arguments, or startup metadata.
- Bind the capability server-side to the primary instance and canonical parent session.
- Enforce request and response size limits, deadlines, schema versions, and deny-by-default methods.
- Revoke the capability when the primary exits or is replaced.

### V1 tool surface

V1 exposes one read-only tool, `agents_status`. Its model-visible input is an optional `run_id`; parent/session scope is injected by the authenticated bridge. Its result is the canonical bounded `ManagedAgentRunProjection`. Raw transcripts, command payloads, transport handles, internal events, and credentials are excluded.

Mutation tools remain deferred until their approval, idempotency, audit, and cancellation semantics are individually specified.

## Implementation Notes

### Constraints

- Preserve Auspex as supervisor/gateway; no direct primary-to-worker transport bypass
- V1 delegation depth is exactly one: only primary may delegate
- All lifecycle/accounting operations distinguish run_id from worker_id
- Raw worker transcript is diagnostic-only; normalized typed events are authoritative UI/integration input
- Every run must have finite budgets and explicit scope
- Operator intervention must be observable by primary
- Large results use artifact references, not unbounded transcript injection
- Use existing tweak.cn/semantic UI tokens for role/status presentation; do not add raw component colors
