---
title: Auspex ↔ Omegon contract drift audit
status: seed
tags: [auspex, omegon, ipc, websocket, contract]
---

# Auspex ↔ Omegon contract drift audit

## Purpose

Track the live control/event contract between Auspex and Omegon so transport evolution in Omegon does not silently desynchronize the Auspex client.

## Live Omegon baseline checked

Checked against Omegon `main` after `v0.15.10-rc.34`, including:

- `becf1eea` — `fix(ipc): make auspex control-plane payloads authoritative`
- `ff6e11f4` — `feat(omegon): route remote slash commands through canonical runtime executor`
- `6bd04b74` — `feat(ipc): add canonical omegon instance descriptor`
- `342b1821` — `feat(tui): add transitional auspex open bridge`

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

- Auspex still consumes websocket-style `OmegonEvent` rather than a typed IPC event stream
- `message_start` / `message_abort` semantics are websocket-only from Auspex’s perspective today
- `WebDashboardStarted` has no Auspex consumer and is transitional Omegon-side bridge metadata

## Next recommended work

1. Add an IPC event client in Auspex and map `IpcEventPayload` directly
2. Maintain websocket compatibility only for remote transitional control
3. Replace degraded remote websocket control with Styrene RPC once the contract stabilizes
