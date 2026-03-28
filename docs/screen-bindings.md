# Screen-to-Control-Plane Binding Matrix

## Purpose

Bind the Auspex screen model to the proposed Omegon control-plane contract.

This is the bridge between product design and implementation.

## Sources

- HTTP snapshot: `ControlPlaneStateV1`
- Graph route: `/api/graph`
- Live event stream and commands: `/ws`

## Screen bindings

### Simple mode shell

#### Header bar
**Uses:**
- `session.cwd`
- `session.git`
- `session.stats`
- `health.status`
- selected compact fields from `harness.routing` and `harness.providers`

**Update model:**
- snapshot-driven
- event-triggered refresh on connection/run state changes

#### Transcript pane
**Uses:**
- WebSocket transcript events
  - message start/chunk/end
  - turn start/end
  - system notifications

**Update model:**
- fully event-driven

#### Activity strip / details tray
**Uses:**
- tool events from WebSocket
- `cleave` summary
- `designTree.focused`
- `openspec` summary
- selected `harness` degradation/recovery state

**Update model:**
- event-driven primary
- snapshot-backed on reconnect

#### Composer
**Uses:**
- WebSocket commands
  - `user_prompt`
  - `cancel`
  - `slash_command` (if surfaced)

**Update model:**
- command-driven

### Power mode: Chat

**Uses:**
- same transcript stream as Simple mode
- richer tool event rendering
- optional thinking events
- selected current-run state from snapshot

### Power mode: Graph

**Uses:**
- `/api/graph`
- selected node detail from `designTree.nodes`
- focused node from `designTree.focused`

**Update model:**
- fetch graph on connect and when graph-invalidating lifecycle changes occur
- use snapshot data for detail panels

### Power mode: Work

**Uses:**
- `designTree.focused`
- `designTree.implementing`
- `designTree.actionable`
- `openspec`
- `cleave`

**Update model:**
- snapshot-driven
- event-triggered refresh on lifecycle/progress changes

### Power mode: Session

**Uses:**
- `harness`
- `health`
- `session`

**Update model:**
- snapshot-driven
- event-triggered refresh when `HarnessStatusChanged` or equivalent state changes occur

## Simple-mode escalation bindings

### Escalate when connection degrades
**Uses:**
- `health.status`
- websocket disconnect/reconnect state

### Escalate when long-running work is active
**Uses:**
- tool lifecycle events
- `cleave.active`
- `cleave.children`

### Escalate when backend routing/fallback changes matter
**Uses:**
- `harness.providers`
- `harness.routing`
- harness-related status events

### Escalate when current work context changes materially
**Uses:**
- `designTree.focused`
- `openspec.changes`

## Implementation rule

Simple and Power mode must derive from the same underlying state/cache. Power mode adds visibility and surfaces; it must not create a second state model.
