# Auspex ↔ Omegon Control Plane

## Goal

Define the normalized Omegon backend contract Auspex should target.

This contract must be tied to versioned Omegon releases rather than an implicit moving backend target.

The current Omegon implementation already exposes useful routes and event streams, but the public contract should be stabilized before the Dioxus client hardens against backend drift.

## Current backend boundary

Omegon already exposes:
- `GET /api/state`
- `GET /api/graph`
- `WS /ws`

These are implemented in:
- `omegon/core/crates/omegon/src/web/mod.rs`
- `omegon/core/crates/omegon/src/web/api.rs`
- `omegon/core/crates/omegon/src/web/ws.rs`

## Proposed public snapshot

```json
{
  "schemaVersion": 1,
  "session": {},
  "designTree": {},
  "openspec": {},
  "cleave": {},
  "harness": {},
  "health": {}
}
```

## Sections

### `session`
- cwd
- pid
- startedAt
- server addr/base URL
- turns
- toolCalls
- compactions
- git branch/detached

### `designTree`
- counts
- focused node
- implementing nodes
- actionable nodes
- nodes inventory

### `openspec`
- active changes
- totalTasks
- doneTasks

### `cleave`
- active
- totalChildren
- completed
- failed
- children

### `harness`
Backed primarily by Omegon's existing `HarnessStatus` model:
- persona/tone
- routing
- providers
- MCP servers
- secrets backend
- inference backends
- container runtime
- memory status
- feature availability
- active delegates

### `health`
- status
- lastUpdatedAt
- protocol/server availability markers

## Why `harness` stays unified in v1

Older web-ui work split some of this into separate `models` and `memory` sections. That has a problem: the actual Omegon code already has a coherent `HarnessStatus` contract that groups this information sensibly. Splitting it early would create avoidable churn between backend and client.

## WebSocket responsibilities

Use WebSocket for:
- transcript events
- tool lifecycle events
- phase/progress events
- harness status updates
- prompt submission
- slash commands
- cancel
- explicit snapshot refresh requests

Do not add write-capable HTTP mutation routes for v1.

## Backend deltas needed before Auspex hardens against it

1. Normalize `/api/state` to the public snapshot shape.
2. Include `HarnessStatus` in the snapshot.
3. Add `schemaVersion`.
4. Add machine-readable startup/discovery for the local server.
5. Document the WebSocket protocol as a public contract.

## Recommended machine-readable startup shape

```json
{
  "pid": 12345,
  "cwd": "/path/to/repo",
  "addr": "127.0.0.1:7842",
  "token": "...",
  "schemaVersion": 1
}
```

Auspex should use that to launch or attach cleanly without scraping human-oriented logs.
