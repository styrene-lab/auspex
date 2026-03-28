# Omegon Backend Delta Checklist for Auspex

## Purpose

Turn the Auspex control-plane requirements into concrete Omegon backend work.

This is not a speculative API wishlist. It is the minimal backend stabilization pass required before the Auspex client should harden against Omegon's current web/control-plane surface.

## Current reality

Omegon already exposes a meaningful local control-plane surface:
- `GET /api/state`
- `GET /api/graph`
- `WS /ws`

The problem is not absence. The problem is that the current implementation surface is narrower and less formal than the contract Auspex should rely on.

## Required backend changes

### 1. Replace implementation-shaped `StateSnapshot` with a public `ControlPlaneStateV1`

#### Current issue
The current snapshot shape in Omegon is implementation-shaped and incomplete for a client that expects a stable contract.

#### Required change
Define a public snapshot type with at least:
- `schemaVersion`
- `session`
- `designTree`
- `openspec`
- `cleave`
- `harness`
- `health`

#### Why
Auspex needs a stable contract, not a dashboard-internal serialization artifact.

---

### 2. Include `HarnessStatus` in the snapshot

#### Current issue
`HarnessStatus` is already a meaningful cross-surface contract in Omegon, but it currently arrives primarily as a WebSocket-side status update rather than a guaranteed part of the main HTTP snapshot.

#### Required change
Include the latest harness status in `GET /api/state` under `harness`.

#### Why
Auspex needs a coherent initial snapshot on connect and cannot depend on racing an initial HTTP fetch against a later `HarnessStatusChanged` event.

---

### 3. Add `schemaVersion`

#### Current issue
There is no explicit version marker on the main state snapshot.

#### Required change
Add `schemaVersion: 1` to the public snapshot.

#### Why
The client needs a migration boundary before multiple app targets start depending on the backend.

---

### 4. Normalize naming to domain-facing public names

#### Current issue
The current shape uses implementation-local names like `design` and `all_nodes`.

#### Required change
Public contract should use:
- `designTree`
- `nodes`

#### Why
Those names match the actual domain language already used elsewhere in Omegon docs/specs and are better suited for a public client contract.

---

### 5. Add machine-readable startup/discovery output

#### Current issue
Current startup/open behavior is operator/TUI oriented, not client-process oriented.

#### Required change
Add a machine-readable control-plane startup mode, for example:

```json
{
  "pid": 12345,
  "cwd": "/path/to/repo",
  "addr": "127.0.0.1:7842",
  "token": "...",
  "schemaVersion": 1
}
```

#### Why
Auspex should not scrape human logs or depend on incidental startup text.

---

### 6. Treat `/api/graph` as a public contract, not an implementation detail

#### Current issue
The route exists, but its stability and field meaning are not yet documented as a public API.

#### Required change
Document and pin:
- node fields
- edge fields
- `group` meaning
- edge type values

#### Why
Graph clients become brittle quickly if enum meanings are implicit.

---

### 7. Pin the WebSocket protocol as a public contract

#### Current issue
The WebSocket protocol is implemented in code but not yet fully hardened as a documented contract.

#### Required change
Document:
- auth expectations
- command message types
- event message types
- initial snapshot behavior
- reconnect expectations
- error handling expectations

#### Why
Auspex should target an API contract, not reverse-engineer source files.

---

### 8. Keep HTTP read-only and WebSocket command-oriented for v1

#### Current issue
There is a temptation to add write-capable HTTP routes once a GUI client exists.

#### Required change
Do not add mutation-oriented HTTP endpoints for Auspex v1.

Use:
- HTTP for state reads
- WebSocket for live events and commands

#### Why
This is already the right seam and avoids unnecessary API sprawl.

---

### 9. Add read-only slice routes after the normalized snapshot lands

#### Recommended routes
- `GET /api/design-tree`
- `GET /api/openspec`
- `GET /api/cleave`
- `GET /api/harness`
- `GET /api/health`

#### Why
These are useful for:
- cheaper refreshes
- targeted debugging
- future client decomposition

These are secondary to stabilizing `/api/state`.

## Suggested Omegon file targets

### Primary
- `omegon/core/crates/omegon/src/web/api.rs`
- `omegon/core/crates/omegon/src/web/mod.rs`
- `omegon/core/crates/omegon/src/web/ws.rs`
- `omegon/core/crates/omegon/src/status.rs`
- `omegon/core/crates/omegon/src/main.rs`

### Likely supporting types
- `omegon/core/crates/omegon/src/tui/dashboard.rs`
- `omegon/core/crates/omegon-traits/src/lib.rs`

## Recommended implementation order

1. Introduce `ControlPlaneStateV1` and migrate `/api/state`.
2. Include `harness` in the snapshot.
3. Add `schemaVersion`.
4. Add startup/discovery JSON output.
5. Write the public WebSocket and graph contract docs.
6. Add slice routes if still justified.

## Constraint

Do not let Auspex absorb backend instability by normalizing everything client-side. That would solve the wrong problem in the wrong layer.
