---
id: omegon-web-single-agent-chat-surface
title: "Omegon Web single-agent chat surface"
status: exploring
tags: [omegon-web, auspex, daemon, chat-ui]
open_questions:
  - "[assumption] Persistent daemon agents can advertise an explicit chat/web capability descriptor rather than requiring Auspex to infer chat eligibility from endpoint reachability."
dependencies: []
related: []
---

# Omegon Web single-agent chat surface

## Overview

Omegon Web is the daemon-owned single-page chat UI for one persistent Omegon runtime. It must work standalone when served by an Omegon daemon and also serve as the target Auspex opens or proxies when an operator chooses to chat with a persistent agent. Auspex owns fleet discovery/routing; Omegon Web owns transcript, composer, session resume, runtime controls, tool approvals, and single-agent interaction state.

## Decisions

### Name the single-agent hosted web UI Omegon Web

**Status:** accepted

**Rationale:** The previous Omnis product/name split was OBE. Omegon Web is descriptive, keeps ownership with Omegon, and avoids creating a premature third product between Omegon and Auspex.

### Omegon Web is daemon-owned and must work standalone

**Status:** accepted

**Rationale:** The single-agent UI should be served by the Omegon daemon itself and usable without Auspex. This preserves the dependency direction: Omegon owns local runtime interaction; Auspex optionally discovers, opens, or proxies it.

### Auspex routes to Omegon Web instead of reimplementing per-agent chat

**Status:** accepted

**Rationale:** For persistent daemon agents, Auspex should expose a Chat action that opens or reverse-proxies the daemon's Omegon Web surface. This keeps fleet concerns in Auspex while keeping transcript rendering, streaming, runtime controls, approvals, and session state close to the executing daemon.

## Backend Surface Delta

Inspection of `../omegon-secundus` found the right seam for Omegon Web: semantic projections under `core/crates/omegon/src/surfaces/*` and inbound UI actions under `core/crates/omegon/src/ui_runtime/actions.rs`. Omegon Web should serialize those surfaces/actions over daemon HTTP/WebSocket rather than porting Ratatui widgets.

### Existing backend seams in omegon-secundus

- Embedded web server shape in `core/crates/omegon/src/web/mod.rs`: `GET /`, `GET /api/startup`, `GET /api/healthz`, `GET /api/readyz`, `GET /api/state`, `WS /ws`, and optional `WS /acp`.
- ACP WebSocket transport in `core/crates/omegon/src/web/acp_ws.rs` with token auth and per-connection session workers.
- Renderer-neutral surface modules: conversation, editor, command, command_menu, dashboard, footer, instruments, memory_status, operations, settings, profile, palette, activity, inline, and layout.
- Renderer-neutral UI actions: submit prompt, continuation, cancel active turn, permission response, operator-wait response, slash command, UI preset/surface visibility, segment select/detail/copy, and copy latest assistant response.

### Native Omegon Web calls to add or confirm

| Call | Purpose |
|---|---|
| `GET /api/web/surfaces` | Full current semantic surface snapshot for the selected session. |
| `WS /api/web/surfaces/stream` | Incremental surface/runtime events for transcript, tools, approvals, turns, and status. |
| `POST /api/web/actions` | Serialized `UiAction` transport for prompts, cancellation, approvals, slash commands, settings/surface changes, and segment actions. |
| `GET /api/web/sessions` | List resumable sessions for reload, switching, and Auspex deep links. |
| `GET /api/web/sessions/{session_id}` | Session metadata plus current surface snapshot. |
| `POST /api/web/sessions` | Create or resume a session. |
| `GET /api/web/capabilities` | Explicit chat/web capability descriptor for Auspex; do not infer eligibility from open ports. |
| `GET /api/web/launch-context` | Direct/proxied/Auspex launch context and back-link/policy-owner metadata. |
| `POST /api/web/attachments` | Browser-safe upload/staging path for files/images. |
| `GET /api/web/attachments/{id}` | Retrieve staged attachment/image content when safe. |

### Web-specific deltas

- Browser reconnect/resume and stale-stream recovery.
- URL-addressable sessions and segment anchors.
- Auspex direct/proxied launch context.
- Browser notifications/tab attention for approval-needed and turn-complete states.
- Browser-safe file uploads instead of `PathBuf` attachments.
- Clipboard actions returned to browser rather than executed server-side.
- Accessibility semantics, focus management, and mobile drawer behavior.

A matching implementation note for the Omegon agent was written to `../omegon-secundus/WEB-UI-WORK-pre-0-27-0.md`.

## Open Questions

- [assumption] Persistent daemon agents can advertise an explicit chat/web capability descriptor rather than requiring Auspex to infer chat eligibility from endpoint reachability.
