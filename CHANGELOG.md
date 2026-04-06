# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
