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

## v1 product direction

- **Default:** simple chat-first interface
- **Toggle:** power-user mode exposing the full Omegon surface
- **Backend:** local Omegon control-plane (`/api/state`, `/api/graph`, `/ws`)
- **Frontend target:** Dioxus across desktop and mobile (desktop-first MVP, mobile-targeted architecture)

## Notes

This is intentionally documentation-first. The backend contract and screen model need to settle before a Dioxus app skeleton is worth creating.
