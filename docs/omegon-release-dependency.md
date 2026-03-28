# Omegon Release Dependency Policy

## Purpose

Auspex must have a hard, explicit dependency link to versioned Omegon releases.

This is both a product and engineering requirement. Auspex is not a generic UI floating above an unstable backend; it is a client for a specific control-plane contract implemented by Omegon. That contract must be versioned and released in a way Auspex can declare, verify, and enforce.

## Policy

### 1. Auspex depends on released Omegon versions, not moving branches

Auspex must target versioned Omegon releases or release candidates, not arbitrary commits or whatever happens to be on `main`.

Accepted dependency targets:
- stable release tags, e.g. `v0.16.0`
- release candidate tags, e.g. `v0.16.0-rc.1`

Rejected as the normal dependency model:
- unpinned branch heads
- undocumented local snapshots
- backend behavior inferred from source drift

## 2. The dependency must be declared in Auspex itself

Auspex should carry an explicit compatibility declaration, for example:

```toml
[omegon]
minimum_version = "0.16.0"
maximum_tested_version = "0.16.x"
control_plane_schema = 1
```

The exact file format can be decided later, but the principle is fixed: Auspex must know which Omegon releases it supports.

## 3. Omegon must expose enough version/protocol identity for runtime verification

At connection or launch time, Auspex should be able to verify at least:
- Omegon version
- control-plane schema version
- optionally protocol capabilities

That identity should be available through machine-readable startup/discovery output and/or the main state snapshot.

## 4. Runtime compatibility should fail clearly

If Auspex connects to an incompatible Omegon version, it should not attempt best-effort silent operation.

It should fail clearly with a message like:
- `Auspex requires Omegon >= 0.16.0 with control-plane schema 1`

This is better than a partially broken client pretending to work.

## Why this is required

Without a hard release dependency:
- the backend contract will drift invisibly
- desktop and mobile clients will encode accidental assumptions
- debugging compatibility problems will become guesswork
- release management across Auspex and Omegon will be sloppy

That approach has a problem: it turns the client/backend boundary into a social convention instead of an engineering contract.

## Recommended Omegon-side additions

To support this policy, Omegon should expose:
- application version
- control-plane schema version
- startup/discovery metadata
- stable route and WebSocket contract docs

## Recommended Auspex-side additions

Auspex should eventually include:
- a compatibility manifest
- version checks during attach/launch
- a user-visible compatibility error screen
- release notes that call out backend version requirements

## Release coupling model

Auspex and Omegon do not need to be in the same repo, but they do need explicit release coupling.

A practical model is:
- Omegon releases `vX.Y.Z`
- Auspex declares support for a bounded range of Omegon releases
- changes to the control-plane contract require a schema-version or compatibility-policy update

## Initial rule for MVP

For the first usable Auspex MVP, require a single pinned Omegon release line and a single control-plane schema version.

That is stricter than necessary long-term, but it is the right discipline while the contract is still settling.
