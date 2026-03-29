# Auspex

Dioxus desktop and mobile interface for Omegon.

## Current purpose

This directory is the start of the Auspex repo path inside the Black Meridian workspace. The initial focus is to turn the product/design thinking into implementation-ready artifacts before code exists.

## Initial structure

- `docs/vision.md` — product positioning, mode model, MVP scope
- `docs/control-plane.md` — proposed Omegon backend contract for Auspex
- `docs/mvp-plan.md` — phased path from design to implementation
- `docs/omegon-release-dependency.md` — policy for tracking versioned Omegon releases
- `docs/compatibility-handshake.md` — runtime verification model for Omegon compatibility
- `docs/release-coordination.md` — release coupling model between Auspex and Omegon
- `docs/error-empty-states.md` — operator-facing loading, degraded, and empty-state behavior
- `docs/app-skeleton-readiness.md` — criteria for when a Dioxus app skeleton is justified
- `docs/mock-fixture-strategy.md` — path from scenario scaffolding to reusable host-session fixtures
- `docs/controller-architecture.md` — role and evolution path for the app controller layer
- `docs/session-source-model.md` — next abstraction step for swapping mock and runtime session sources
- `docs/session-source-implementation-notes.md` — implementation note for introducing SessionSource in code
- `docs/session-source-transition-note.md` — note that SessionSource is now the highest-value next refactor
- `docs/embedded-runtime-model.md` — bundled subsystem model for Omegon and Styrene under Auspex
- `docs/supervision-startup-states.md` — host lifecycle and supervision states for embedded subsystems
- `docs/remote-connection-model.md` — desktop-hosted remote phone connection strategy using Styrene
- `docs/styrene-relay-session-model.md` — desktop-hosted session abstraction for phone clients
- `docs/phone-command-event-surface.md` — initial semantic relay surface for phone commands and events
- `docs/host-projection-model.md` — how the desktop host reduces Omegon state for phone clients
- `docs/phone-simple-mode-projection.md` — minimum phone Simple mode state projection
- `docs/relay-state-machine.md` — lifecycle and transitions for the desktop relay host
- `docs/host-event-projection-rules.md` — when host/backend changes produce phone-facing updates
- `docs/auspex-in-the-stack.md` — Auspex-specific role within the broader Black Meridian stack doctrine

## v1 product direction

- **Default:** simple chat-first interface
- **Toggle:** power-user mode exposing the full Omegon surface
- **Backend:** local Omegon control-plane (`/api/state`, `/api/graph`, `/ws`)
- **Frontend target:** Dioxus across desktop and mobile (desktop-first MVP, mobile-targeted architecture)

## Release cadence

Auspex should establish release guardrails early rather than inventing them later under pressure.

Current release workflow is scaffolded via `Justfile` and follows a simple line:
- validate locally (`just lint`)
- cut RCs from `main` (`just rc`)
- cut stable releases from RC versions (`just release`)
- advance to the next dev cycle (`just next`)

The exact CI/release pipeline can evolve later, but the version and tagging cadence should remain explicit from the beginning.

## Notes

This started documentation-first, but now includes a minimal Dioxus scaffold proving the basic conversation shell. The backend contract and remote-runtime layers still need to settle before a real Omegon/Styrene integration should harden.

## Bootstrap paths

Auspex now supports two early remote bootstrap seams before full live transport hardens:

- `AUSPEX_REMOTE_SNAPSHOT_PATH=/path/to/state.json` — load an Omegon-shaped snapshot from disk
- `AUSPEX_OMEGON_STATE_URL=http://127.0.0.1:7842/api/state` — fetch Omegon over HTTP at startup
- `AUSPEX_OMEGON_STARTUP_URL=http://127.0.0.1:7842/api/startup` — optional explicit startup discovery override
- `AUSPEX_OMEGON_WS_URL=ws://127.0.0.1:7842/ws` — optional explicit event-stream override when discovery is unavailable
- `AUSPEX_OMEGON_WS_TOKEN=...` — optional fallback WebSocket auth token appended as `?token=` when missing

Behavior is intentionally simple:
- snapshot file wins if both are set
- HTTP bootstrap is opt-in
- when available, Auspex prefers Omegon startup discovery at `/api/startup`
- startup discovery supplies the canonical state URL, WS URL, token, and auth mode/source
- Auspex currently requires control-plane schema `1` and treats other schema versions as a visible compatibility failure
- if discovery is unavailable, Auspex falls back to the configured state URL and derived `/ws`
- if `AUSPEX_OMEGON_WS_TOKEN` is set, Auspex appends it to the fallback WebSocket URL unless a `token` query is already present
- if bootstrap fails, Auspex falls back to the mock local session and surfaces the failure in the UI
