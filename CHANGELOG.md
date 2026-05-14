# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
- Removed the inactive Scribe workspace from active navigation; Flynt remains the task board, Sentry remains execution, and Auspex focuses on workflow handoff, command, and observability surfaces.
- Reframed the active Graph workspace around deployed-agent topology instead of design-tree/tasking state, preserving Flynt as the task/board owner.
- Added ACP endpoint plumbing and reframed the Chat workspace as an ACP session surface for config, commands, plans, tool calls, and assistant deltas.

## [0.2.0-rc.1] - 2026-04-07

### Changed
- Replaced the previous rail-dominant shell with a cockpit-oriented scaffold built around a dark grid-dot canvas, persistent Auspex/Attached Omegon/Deployment/Activity truth panels, a focus host, and a contextual detail region.
- Preserved controller/state plumbing and existing routed workspace surfaces while re-composing them into the new cockpit shell rather than inheriting the old left/right/center layout semantics.


## [0.1.0-rc.6] - 2026-04-07

### Changed
- Embedded/local Auspex control remains IPC-only, but remote Omegon attachments temporarily retain limited websocket command dispatch until Styrene RPC replaces that path.

### Fixed
- Release-candidate transport policy now matches the intended boundary: embedded Omegon uses IPC, while remote web-compat control still works over websocket during the transition.

## [0.1.0-rc.5] - 2026-04-07

### Changed
- Embedded Auspex-to-Omegon desktop control now routes commands over IPC instead of treating the websocket event stream as a fallback command path.
- Detached remote sessions now surface an honest "Detached host session" route label instead of pretending the operator is targeting a local shell.

### Fixed
- Settings/provider inventory now merges desktop auth-bridge metadata with runtime provider metadata without dropping model identity during refresh or auth actions.
- Release candidate now includes the IPC transport dependencies required for the embedded control path to build from HEAD.

## [0.1.0-rc.4] - 2026-04-06

### Added
- Controller-owned operator readiness model and global startup/convergence layer covering embedded Omegon startup, session snapshot readiness, auth inventory loading, and prompt execution readiness.
- Dedicated Session workspace so generic session/control-plane inspection no longer lives under the Scribe tab.

### Changed
- Scribe workspace is now an honest placeholder for future cross-repo Scribe platform integration instead of a mislabeled session inspector.
- Auth refresh now rehydrates the live remote prompt-execution gate, not just the visible Settings/provider UI.

### Fixed
- Provider auth refresh preserves existing provider model metadata instead of dropping it to `unreported` during auth-state updates.
- Embedded-omegon chat readiness now converges through one synchronized auth/provider state path rather than split UI-vs-session truth.

## [0.1.0-rc.3] - 2026-04-06

### Changed
- Warning/blocked UI surfaces now use a cleaner amber palette instead of muddy brown warning tones.
- Chat composer now swaps dead disabled input for a provider-setup callout when no authenticated providers are available.
- Successful provider auth now closes Settings, returns focus to Chat, and surfaces a provider-ready confirmation notice.

### Fixed
- Opening Settings now refreshes desktop auth status immediately, so provider cards/actions hydrate from the auth bridge instead of appearing inert behind stale empty inventory.
- Composer submit gating now respects the effective provider inventory shown to the operator, not just the raw remote session model.

## [0.1.0-rc.2] - 2026-04-06

### Added
- Instance-registry-backed lifecycle state engine with freshness tracking, stale/lost handling for detached services, and controller write-back for attached-instance mutations.
- Controller-owned telemetry snapshot cache plus a dedicated telemetry aggregation module for lifecycle, provider, and control-plane rollups.
- Settings operator surface with live auth bridge actions and slash-command-backed provider auth controls.
- Audit timeline support for telemetry entries, including structured telemetry rollup change records in the existing append-only ledger.

### Changed
- Command routing now uses instance-targeted envelopes instead of singleton JSON command assumptions.
- Session telemetry now projects cross-instance lifecycle, provider, and control-plane rollups rather than only selected-route summaries.
- Top chrome, tabs, badges, rails, and settings cards use tighter radii for a less pillowy operator-console feel.

### Fixed
- Remote chat no longer reports ready/sendable state when Omegon has no authenticated providers.
- Telemetry extraction and expanded schema fixtures are reconciled cleanly across controller, screens, and tests.
- Audit telemetry entries are append-only via versioned sequence keys instead of being silently deduped by stable IDs.

## [0.1.0-rc.1] - 2026-04-04

### Added
- Structured turn/block transcript rendering for live Omegon sessions.
- Typed session activity strip semantics for running, waiting, degraded, completed, and failure states.
- Typed top-level startup, reconnecting, bootstrap, and failure surface notices.
- Session-screen context usage visibility.
- Initial release-candidate framework with changelog, release preflight, release manifest generation, and tag-driven CI release workflow.

### Changed
- Remote session projection now preserves richer live-session semantics instead of flattening everything into chat messages.
- Release planning now follows an explicit RC/stable framework toward `0.1.0`.
- Local release commands now align with changelog and preflight policy.

### Fixed
- Thinking chunks remain distinct from assistant response text.
- Message aborts remain visible instead of disappearing silently.
- Tool updates stream into persistent transcript cards.
- Release documentation and compatibility placeholders now reflect current schema/version policy.

## [0.0.1-rc.6] - 2026-04-04

### Added
- Release-candidate milestone documenting the repository state immediately before formal release framework setup.
