# Auspex

Auspex is the first-party operator shell for Omegon and Styrene: a Dioxus-based desktop-first control surface for attached sessions, embedded runtimes, transport-aware command routing, and operator-facing telemetry.

## Current status

Auspex is an active Rust application repo, not a design-only placeholder.

Current reality:
- desktop-first Dioxus app in `src/`
- release-candidate line currently at `0.2.0-rc.1` in `Cargo.toml`
- explicit RC/stable release workflow with changelog + release manifest
- remote/bootstrap compatibility checks against Omegon schema + semver metadata
- embedded/local IPC-first control path with remote/transitional websocket compatibility still present where required
- design and OpenSpec artifacts retained in `docs/` and `openspec/` so implementation stays traceable

## Repository layout

- `src/` — Dioxus desktop/web shell and current operator workspaces: Cop, Chat, Session, Scribe, Graph, and Audit
- `auspex-core/` — reusable control-plane types, fixtures, bootstrap/discovery, command transport, event streams, telemetry, and state machines
- `auspex-operator/` — Kubernetes operator for Omegon agent fleet management, CRDs, reconciliation, fleet API, and embedded MQTT broker
- `tools/keygen/` — key-generation utility
- `pkl/` and `profiles/` — deployment/profile configuration inputs
- `docs/` — long-lived design docs and architecture notes; start with `docs/README.md` for status and navigation
- `openspec/changes/` — change proposals and delta specs
- `scripts/` — release-manifest and preflight helpers
- `.github/workflows/` — CI and release automation
- `site/` — preliminary Astro marketing/docs site scaffold for future Cloudflare Pages hosting

## Development

Prerequisites:
- Rust stable
- Node.js 22+ for the site scaffold
- sibling Omegon checkout at `../omegon` because `omegon-traits` is a path dependency

Typical commands:
```bash
just check
just test
just validate
cargo run
```

## CI and path dependency note

Auspex depends on:
```toml
omegon-traits = { path = "../omegon/core/crates/omegon-traits" }
```

That means local development and CI both need a sibling `omegon` checkout. GitHub Actions handles this by checking out `styrene-lab/omegon` and symlinking it into the expected sibling path before running Cargo.

## Bootstrap paths

Auspex currently supports these early bootstrap seams while live transport hardens:

- `AUSPEX_REMOTE_SNAPSHOT_PATH=/path/to/state.json` — load an Omegon-shaped snapshot from disk
- `AUSPEX_OMEGON_STATE_URL=http://127.0.0.1:7842/api/state` — fetch Omegon state over HTTP at startup
- `AUSPEX_OMEGON_STARTUP_URL=http://127.0.0.1:7842/api/startup` — optional startup discovery override
- `AUSPEX_OMEGON_WS_URL=ws://127.0.0.1:7842/ws` — optional websocket event-stream override when discovery is unavailable
- `AUSPEX_OMEGON_WS_TOKEN=...` — optional fallback websocket auth token appended as `?token=` when missing

Behavior summary:
- snapshot file wins if both snapshot and HTTP bootstrap are set
- startup discovery is preferred when available
- Auspex enforces control-plane schema compatibility and Omegon version policy at runtime
- if bootstrap fails, Auspex falls back to the mock local session and surfaces the failure in the UI

## Release workflow

Auspex follows an explicit RC/stable process:
- maintain `CHANGELOG.md`
- cut RC tags like `vX.Y.Z-rc.N` — current line is `v0.2.0-rc.N`
- promote stable tags like `vX.Y.Z` — current stable target is `v0.2.0`
- run `python3 scripts/release_preflight.py` before stable promotion
- let GitHub Actions build archives, checksums, and `release-manifest.json`

Useful local commands:
```bash
just rc
just release
just next
```

## Preliminary site scaffold

`site/` contains a basic Astro project intended for future Cloudflare Pages hosting. It is deliberately dormant right now:
- no Pages project has been launched from this repo by this change
- the scaffold exists so repo structure, CI, and future docs/marketing hosting can converge cleanly

Build it locally with:
```bash
cd site
npm install
npm run dev
npm run build
```
