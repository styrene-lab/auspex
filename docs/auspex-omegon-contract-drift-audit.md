---
title: Auspex ↔ Omegon contract drift audit
status: seed
tags: [auspex, omegon, ipc, websocket, contract]
---

# Auspex ↔ Omegon contract drift audit

## Purpose

Track the live control/event contract between Auspex and Omegon so transport evolution in Omegon does not silently desynchronize the Auspex client.

## Live Omegon baseline checked

Checked against Omegon `v0.15.25`. IPC contract (event payloads, methods, state snapshot) core is unchanged from v0.15.10. Additions in 0.15.24: `family.vital_signs` event, daemon session router state, vox extension bridge (uses existing `DaemonEventEnvelope` — no new wire events). Additions in 0.15.25: agent loop churn reduction (no wire changes).

## Transport boundary

- **Embedded/local full control:** authoritative IPC control plane
- **Remote transitional control:** degraded websocket command bridge
- **Future remote control:** Styrene RPC

## Omegon event/control surfaces

### Authoritative IPC event payloads

From Omegon `IpcEventPayload` and IPC projection:

- `turn.started`
- `turn.ended`
- `message.delta`
- `thinking.delta`
- `message.completed`
- `tool.started`
- `tool.updated`
- `tool.ended`
- `agent.completed`
- `phase.changed`
- `decomposition.started`
- `decomposition.child_completed`
- `decomposition.completed`
- `family.vital_signs` (0.15.24+)
- `harness.changed`
- `state.changed`
- `system.notification`
- `session.reset`

### Websocket / legacy AgentEvent compatibility surface still relevant to Auspex

Auspex currently consumes websocket-style JSON events via `OmegonEvent`:

- `state_snapshot`
- `message_start`
- `message_chunk`
- `thinking_chunk`
- `message_end`
- `message_abort`
- `system_notification`
- `harness_status_changed`
- `session_reset`
- `turn_start`
- `turn_end`
- `tool_start`
- `tool_update`
- `tool_end`
- `agent_end`
- `phase_changed`
- `context_updated`
- `decomposition_started`
- `decomposition_child_completed`
- `decomposition_completed`
- `slash_command_result` (handled separately in controller)

## Known asymmetries

### IPC does not currently project all websocket/TUI-facing AgentEvents

Omegon IPC projection explicitly omits these `AgentEvent` variants:

- `MessageStart`
- `MessageAbort`
- `ContextUpdated`
- `WebDashboardStarted`

Implication: Auspex must not assume IPC parity for those events. If Auspex adopts IPC event streaming later, it will need either:

1. equivalent IPC payloads added upstream, or
2. local fallback semantics for those missing event classes.

### ContextUpdated drift fixed in Auspex

Omegon now emits authoritative `ContextUpdated` with:

- `tokens`
- `context_window`
- `context_class`
- `thinking_level`

Auspex previously modeled only `tokens`. This audit cycle updated `OmegonEvent::ContextUpdated` to accept the full shape and store `context_window`.

## Canonical control seams

### IPC methods Auspex should treat as first-class

- `submit_prompt`
- `cancel`
- `run_slash_command`
- state/snapshot methods and typed capabilities

### Transitional websocket control still allowed

- `user_prompt`
- `slash_command`
- `cancel`
- `request_snapshot`
- websocket `slash_command_result`

This is compatibility behavior, not the long-term contract.

## Current Auspex status

### Aligned

- Embedded/local command control prefers IPC
- Remote command control can still use degraded websocket bridge
- Canonical slash contract is reflected in UI dispatch paths
- `context_updated` now accepts `context_window`
- Bootstrap messaging tells operator whether control is IPC or degraded websocket bridge

### Still transitional / watch closely

- Auspex still consumes websocket-style `OmegonEvent` for remote sessions; IPC path is primary for embedded
- `message_start` gap resolved: `MessageDelta` auto-initializes `pending_role` as `Assistant` when no `MessageStart` preceded it (IPC transport omits `MessageStart`)
- `message_abort` gap resolved: `TurnEnded` flushes any uncommitted pending message as an implicit abort block
- `ContextUpdated` remains IPC-omitted — context token display is stale in IPC-only mode (low priority, cosmetic)
- `WebDashboardStarted` has no Auspex consumer and is transitional Omegon-side bridge metadata

## Next recommended work

1. ~~Add an IPC event client in Auspex and map `IpcEventPayload` directly~~ — done: IPC event subscriber, `SessionEvent` adapter, and auto-init/implicit-abort fallbacks are active
2. Maintain websocket compatibility only for remote transitional control
3. Replace degraded remote websocket control with Styrene RPC once the contract stabilizes
4. Resolve `ContextUpdated` gap: either add IPC projection upstream or derive from `state.changed` refresh

## Migration checklist

### Phase 1 — keep current split truthful

- [ ] Keep embedded/local Auspex control routed through IPC methods (`submit_prompt`, `cancel`, `run_slash_command`)
- [ ] Keep remote websocket control explicitly marked degraded/transitional in operator-facing copy
- [ ] Keep bootstrap notes reporting the active control mode (IPC vs degraded websocket bridge)
- [ ] Recheck Omegon `main` before each RC cut that changes transport semantics

### Phase 2 — prepare Auspex for typed IPC events

- [x] Add an Auspex-native IPC event client that can subscribe to typed `IpcEventPayload` frames
- [x] Introduce an internal Auspex event adapter that maps both websocket JSON events and IPC typed events into one normalized UI/event model (`SessionEvent`)
- [x] Add fixture coverage for IPC event paths including auto-init and implicit abort
- [x] Verify slash-command results still surface coherently when command dispatch path is IPC rather than websocket

### Phase 3 — handle current IPC/websocket asymmetries explicitly

- [x] `message_start` gap: `MessageDelta` auto-initializes `pending_role` as `Assistant` when no `MessageStart` preceded it
- [x] `message_abort` gap: `TurnEnded` flushes any uncommitted pending message as an implicit abort block
- [ ] Decide whether Auspex should derive context status from `state.changed` / snapshots when IPC continues omitting `ContextUpdated`
- [x] Treat `WebDashboardStarted` as Omegon-local bridge metadata — no Auspex consumer needed
- [ ] Open upstream Omegon follow-up if `ContextUpdated` IPC projection is needed for live token display

### Phase 4 — Styrene RPC transition

- [ ] Define the remote Styrene RPC command contract in semantic terms rather than reusing websocket JSON envelopes blindly
- [ ] Map current degraded remote websocket commands (`user_prompt`, `slash_command`, `cancel`, `request_snapshot`) onto the future Styrene RPC surface
- [ ] Keep canonical slash execution semantics shared across IPC, websocket bridge, and Styrene RPC transports
- [ ] Remove degraded websocket remote control only after Styrene RPC covers the required remote management operations and Auspex has passing parity tests

### Verification gates

- [ ] Add a transport-parity test matrix that asserts which behaviors are expected on IPC, websocket bridge, and future Styrene RPC
- [ ] Add a contract-drift review step to the release checklist whenever Omegon changes `omegon-traits`, IPC projection, websocket command handling, or startup/control descriptors
- [ ] Update this document whenever Omegon commits change the authoritative IPC payloads, canonical slash executor, or remote compatibility bridge semantics

