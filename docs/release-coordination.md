# Release Coordination Model

## Purpose

Describe how Auspex and Omegon releases should coordinate once the control-plane contract becomes real.

## Principle

Auspex and Omegon may live in separate repositories, but their releases must still be coupled through an explicit compatibility contract.

## Release roles

### Omegon owns
- control-plane implementation
- route and WebSocket behavior
- schema evolution
- startup/discovery identity

### Auspex owns
- compatibility manifest
- runtime handshake and error handling
- client UX against the control plane
- release notes for supported Omegon versions

## Recommended release workflow

### 1. Omegon changes the public control-plane
If the public contract changes:
- update schema docs
- decide whether the schema version changes
- cut a versioned release or release candidate

### 2. Auspex validates against that release line
Auspex updates:
- compatibility manifest
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

## Recommended operator experience

Operators should be able to tell, from release notes alone:
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

This is intentionally conservative. It is the right discipline while the boundary is still stabilizing.
