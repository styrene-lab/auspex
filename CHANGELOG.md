# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
