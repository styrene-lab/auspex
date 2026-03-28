# Omegon Implementation Slice for ControlPlaneStateV1

## Goal

Identify the smallest practical Omegon implementation slice that would make Auspex's backend dependency concrete.

## Slice definition

The initial implementation slice should do exactly this:

1. Add a public `ControlPlaneStateV1` snapshot type.
2. Move the existing `/api/state` route to that shape.
3. Include `HarnessStatus` in the snapshot.
4. Include `omegonVersion` and `schemaVersion` in the snapshot.
5. Add machine-readable startup/discovery output with the same version markers.

Do not expand scope beyond that until the client actually needs more.

## Why this slice is sufficient

With that slice in place, Auspex can:
- verify compatibility with a released Omegon version
- bootstrap a coherent session view
- render Simple mode from the snapshot
- render Power mode from the same contract plus `/api/graph` and `/ws`

## Suggested implementation order in Omegon

### 1. `web/api.rs`
Introduce `ControlPlaneStateV1` and build it from current runtime handles.

### 2. `status.rs`
Reuse `HarnessStatus` as the `harness` section directly.

### 3. `web/mod.rs`
Ensure server metadata needed for `session.server` and discovery output is available.

### 4. `main.rs`
Add machine-readable startup/discovery mode.

### 5. `web/ws.rs`
No major redesign required immediately; document and preserve the existing command/event seam.

## Test expectations

The first Omegon slice should add or update tests for:
- `/api/state` top-level shape
- presence of `schemaVersion`
- presence of `omegonVersion`
- presence of `harness`
- JSON field naming stability

## Non-goals for this slice

Do not require, yet:
- slice routes
- major WebSocket redesign
- graph route redesign
- mobile-specific protocol behavior
- command authorization beyond current localhost model

## Delivery criterion

Auspex should be able to say:

> I support Omegon release line X with control-plane schema 1.

and Omegon should be able to answer that claim programmatically.
