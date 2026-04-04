# Release Coordination Model

## Purpose

Describe how Auspex and Omegon releases coordinate through an explicit compatibility contract.

## Principle

Auspex and Omegon may live in separate repositories, but their releases must still be coupled through a declared support boundary.

Auspex should copy Omegon's **release invariants** — explicit versioning, prerelease discipline, changelog hygiene, and machine-readable artifacts — without inheriting Omegon's full release complexity prematurely.

## Release roles

### Omegon owns
- control-plane implementation
- route and WebSocket behavior
- schema evolution
- startup/discovery identity

### Auspex owns
- compatibility declaration in `Cargo.toml`
- runtime handshake and error handling
- client UX against the control plane
- release notes for supported Omegon versions

## Current compatibility boundary

Auspex currently declares support for:

- minimum Omegon version: `0.15.7`
- maximum tested Omegon version: `0.15.7`
- required control-plane schema: `2`

Those values live in `Cargo.toml` under `[package.metadata.omegon]` and should move only with deliberate validation.

## Recommended release workflow

### 1. Omegon changes the public control-plane
If the public contract changes:
- update schema docs
- decide whether the schema version changes
- cut a versioned release or release candidate

### 2. Auspex validates against that release line
Auspex updates:
- compatibility metadata
- any binding or rendering assumptions
- release notes / support matrix

### 3. Auspex ships with explicit support bounds
Each Auspex release should declare:
- minimum supported Omegon version
- maximum tested Omegon version
- required control-plane schema version

## Schema policy

### Patch-compatible changes
If the public contract does not break the client model, the schema version may remain the same.

### Breaking changes
If the public contract changes in a way that breaks client expectations, bump the control-plane schema version.

Auspex should then require the new schema explicitly.

## RC framework policy

For the first release line toward `0.1.0`, Auspex uses a deliberately narrow release framework:

- semver RC tags such as `v0.1.0-rc.1`
- stable tags such as `v0.1.0`
- macOS arm64 raw archive artifacts only
- unsigned release artifacts initially
- scripted release preflight for stable promotion
- machine-readable `release-manifest.json`

That scope is intentionally conservative. Trying to ship cross-platform packaging, signing, and downstream automation before the release loop is proven would be the wrong trade.

## Recommended operator experience

Operators should be able to tell, from release notes and repository metadata alone:
- which Omegon line they need
- whether an upgrade is mandatory
- whether they can safely stay on the prior line

## Anti-patterns

Do not rely on:
- branch names
- implicit source compatibility
- undocumented local builds
- manual tribal knowledge of which combinations work

That approach has a problem: it makes compatibility debugging and release support impossible at scale.

## MVP rule

For the first usable release line, keep it strict:
- one control-plane schema
- one declared Omegon release line
- one visible compatibility check path
- one narrow release artifact surface
