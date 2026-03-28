# Auspex Vision

> Dioxus interface for Omegon across desktop and mobile: chat-first by default, full harness surface available behind a persistent power-user toggle.

## Positioning

Auspex is the operator-facing desktop application for Omegon.

It should not start as a separate intelligence backend, a browser-first SaaS, or a thin mdserve wrapper. Omegon already exposes the right local control-plane direction: a localhost server with HTTP state routes and a bidirectional WebSocket command/event stream. Auspex should use that backend boundary and focus on providing a better operator experience.

## Product thesis

Auspex should feel like:
- a simple local chat interface by default
- a notes / diagramming / workbench environment over time
- a full harness console when the operator explicitly asks for it

That means the UI must support two cognitive budgets:
- **Simple mode** — calm, chat-first, minimal telemetry
- **Power-user mode** — exposes graph, lifecycle, telemetry, routing, and system details

## Core decision

### Auspex v1 is a local client, not a new backend

Auspex should be a Dioxus application with one product architecture spanning desktop and mobile.

For the initial MVP, desktop is the first-class launch target because it can:
- launch a local Omegon process for a repository session, or
- attach to an existing local Omegon control-plane server

Auspex should treat Omegon as a black-box local control-plane backend.

Current Omegon boundary to leverage:
- `GET /api/state`
- `GET /api/graph`
- `WS /ws`

### Why desktop-first, but not desktop-only

Desktop is still the right MVP target because:
- process ownership is easier than browser-only orchestration
- local auth/token handoff is simpler
- reconnect and lifecycle semantics are more deterministic
- desktop gives the shortest path to validating the control-plane and two-mode UX

But Auspex should be architected for mobile from the start. The desktop/phone boundary is weak for this product: the default chat-first surface, compact status model, and settings-driven power-user expansion all map naturally onto mobile as well. Dioxus is useful here because it gives one application architecture across desktop and mobile even if the initial MVP ships on desktop first.

## Interface philosophy

### Default behavior: simple chat-first UX

The default Auspex experience should be a clean local chat application with enough context to keep the operator oriented, but without forcing them to understand design-tree, OpenSpec, cleave, or model-routing internals.

The operator should always be able to answer:
- what repo/session am I in?
- is Omegon connected?
- is it working?
- what is it doing in plain language?
- what did it say?

### Optional expansion: power-user mode

Power-user mode should reveal the full control-plane surface:
- graph view
- work/lifecycle view
- session/system diagnostics
- tool stream and richer telemetry
- routing/provider/local inference details
- cleave/delegate visibility

This must be a persistent settings toggle, not a separate app or a hidden developer-only mode.

## Visibility matrix

| Surface | Simple default | Power default | Escalate in Simple when actionable? |
|---|---:|---:|---:|
| Transcript | yes | yes | yes |
| Session connection state | yes | yes | yes |
| Busy/idle / current activity | yes | yes | yes |
| Tool details | collapsed | yes | yes |
| Thinking stream | no | optional | no |
| Design focus summary | limited | yes | yes |
| Design counts | no | yes | limited |
| Node inventory | no | yes | no |
| Graph | no | yes | no |
| OpenSpec summary | limited | yes | yes |
| OpenSpec change list | no | yes | yes |
| Cleave summary | limited | yes | yes |
| Cleave child detail | no | yes | yes |
| Routing/model/provider detail | limited | yes | yes |
| Persona/tone | maybe | yes | yes |
| MCP/secrets/inference detail | no | yes | yes |
| Memory telemetry | no | yes | yes |
| Health diagnostics | no | yes | yes |

## Screen model

### Simple mode

Single chat-first surface:
1. Header bar
2. Transcript pane
3. Activity strip / details tray
4. Composer row

### Power-user mode

Four primary surfaces:
- Chat
- Graph
- Work
- Session

## MVP scope

### Must have
- desktop shell
- local Omegon process launch or attach
- transcript
- prompt input
- cancel
- compact session status
- power-user mode toggle
- graph view
- work view
- session/system view
- remote-phone-oriented connection model with desktop Auspex as the initial bridge point

### Explicitly out of scope for v1
- full notes replacement
- full diagramming system
- direct phone-to-Omegon attachment as the primary mobile path
- native mobile parity in the initial MVP release
- multi-repo workspace management
- embedded document browser
- memory fact graph
- browser-first distribution

## Product rule

There should be one backend contract and two UI projections:
- Simple mode: chat-first, low-cognitive-load
- Power-user mode: full harness visibility
