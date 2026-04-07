---
id: auspex-embedded-operator-parity
title: "Auspex embedded operator parity via canonical command surface"
status: seed
tags: []
open_questions:
  - "What is the minimal instance-targeted command adapter shape Auspex should adopt now so operator settings/actions can bind to a selected Omegon instance/session without assuming a singleton backend?"
  - "What is the exact canonical slash command contract for auth/status/login/logout/unlock — command names, argument encoding, response semantics, and whether instance-target routing is resolved by Auspex or by Omegon's slash executor?"
dependencies: []
related: []
---

# Auspex embedded operator parity via canonical command surface

## Overview

Achieve operator-critical parity for embedded Omegon by routing Auspex desktop settings and control surfaces through Omegon's canonical command/slash execution layer, while preserving N+1 instance supervision under a single Auspex authority.

## Research

### Command routing seam for N+1 Omegon instances

Current Auspex command plumbing has now split by transport mode. Controller-level `TargetedCommand { target: CommandTarget { session_key, dispatcher_instance_id }, command_json }` remains the stable semantic envelope, but desktop embedded/local control routes over IPC while remote attach may still send limited commands over the websocket bridge. This is intentional transitional behavior: the websocket path is degraded compatibility for remote Omegon management until the Styrene RPC contract is established, not the long-term canonical control surface.

### Unified slash surface assessment

Live Omegon startup capabilities now advertise `slash_commands`. The embedded backend contract still treats slash execution as recommended rather than minimum, and current docs expose two adjacent shapes: relay-semantic `run_slash_command` (phone/desktop layer) and websocket-level `slash_command { name, args }` (auspex-data-model-v2 / screen-bindings). Auspex already has canonical slash transport types (`CanonicalSlashCommand`, `TargetedCommand`) and explicit route selection, so the integration seam is ready. What is still not grounded in this repo is the exact canonical auth command naming/argument contract and how instance-targeted slash execution should be bound when multiple Omegon instances are supervised under one Auspex.

### Authoritative unified slash contract

Authoritative contract found in `docs/omegon-unified-slash-contract.md`. Current canonical remote slash request is `name: String, args: String`; WebSocket inbound shape is `{ type: "slash_command", name, args }`; result event is `slash_command_result` with `{ name, args, accepted, output }`. Exact current auth commands are `/auth`, `/auth status`, `/auth unlock`, `/login <provider>`, and `/logout [provider]` with default logout provider `anthropic`. Auspex should bind to this actual contract, not the aspirational richer spec.

### Live transport alignment with Omegon rc.34

Checked against Omegon `v0.15.10-rc.34`: the runtime now has a first-class IPC server under `core/crates/omegon/src/ipc/*`, with `AgentEvent` projected into typed IPC payloads in `ipc/connection.rs`. Auspex should therefore treat IPC as the authoritative full-control transport for embedded/local authority. Remote websocket control remains allowed only as a degraded bridge for attach/manage flows until the Styrene RPC contract replaces it.

## Decisions

### Operator command routing is explicit and selectable

**Status:** accepted

**Rationale:** Auspex must not infer a singleton backend target for parity-critical controls. The operator surface should expose selectable command routes (host control plane vs session dispatcher today) so the selected authority is explicit and can grow into a full instance registry without rewriting the settings/control model.

## Open Questions

- What is the minimal instance-targeted command adapter shape Auspex should adopt now so operator settings/actions can bind to a selected Omegon instance/session without assuming a singleton backend?
- What is the exact canonical slash command contract for auth/status/login/logout/unlock — command names, argument encoding, response semantics, and whether instance-target routing is resolved by Auspex or by Omegon's slash executor?
