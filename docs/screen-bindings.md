# Screen-to-Control-Plane Binding Matrix

## Purpose

Bind the Auspex screen model to the proposed Omegon control-plane contract.

This is the bridge between product design and implementation.

## Sources

- HTTP snapshot: `ControlPlaneStateV1`
- Deployment lifecycle telemetry from the control-plane snapshot
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
- ACP endpoint from Omegon startup/control-plane metadata (`/acp`)
- ACP session updates: assistant deltas, thoughts, plans, tool calls, available commands, and config options
- selected current-run state from snapshot while ACP is not yet attached

**Update model:**
- ACP stream is primary for the interactive session surface
- snapshot state remains the readiness and fallback layer

### Power mode: Graph

**Uses:**
- attached deployment instances from lifecycle telemetry
- active delegate/runtime activity summaries
- dispatcher binding and route identity

**Update model:**
- snapshot-driven topology with event-triggered refresh on lifecycle/activity changes
- use Session and Audit for deeper per-instance and lifecycle detail

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
