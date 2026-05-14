# Auspex docs

This directory holds long-lived design notes and implementation context for Auspex.

Treat this directory as a mixed-status archive: some files describe current implementation, while others are design targets or historical planning notes. When code and docs disagree, the current workspace layout and source modules are authoritative until the relevant doc is refreshed.

Repository-level setup, development commands, bootstrap paths, and release workflow live in the root `README.md`.

## Current implementation anchors

- Root `README.md` — current repo status, workspace layout, development commands, bootstrap paths, and release workflow.
- `../Cargo.toml` — authoritative workspace membership and release-candidate version.
- `../src/` — Dioxus shell and operator workspaces: Cop, Chat, Session, Graph, Workflow, and Audit.
- `../auspex-core/src/lib.rs` — core crate module map for control-plane types, fixtures, bootstrap/discovery, transport, telemetry, and state machines.
- `../auspex-operator/src/main.rs` — operator entry point for CRD watching, reconciliation, fleet API, and embedded MQTT startup.
- `../CHANGELOG.md` — release history and current RC line.

## Start here by task

### Product and operator intent

- `vision.md` — product direction and operator experience goals.
- `auspex-in-the-stack.md` — where Auspex sits relative to Omegon and Styrene.

### Current/control-plane architecture

- `control-plane.md` — Omegon control-plane contract direction.
- `controller-architecture.md` — controller/state architecture notes.
- `compatibility-handshake.md` — schema/version compatibility expectations.
- `omegon-embedded-backend-contract.md` — embedded-backend contract notes.

### Runtime, sessions, and bootstrap

- `embedded-runtime-model.md` — embedded runtime model.
- `auspex-primary-coordinator.md` — primary coordinator posture, fleet-control primitives, and UI implications.
- `auspex-runtime-backends.md` — runtime backend taxonomy.
- `nex-forge-package-lane.md` — how Nex forge/build-image/profile primitives should feed Styrene distributed agent packages.
- `styrene-secret-grant-architecture.md` — how Styrene identity, secrets, RBAC, IPC, content, MQTT, and tunnels compose into backend-agnostic agent secret seeding.
- `librefang-peer-runtime-positioning.md` — LibreFang as a supervised external peer runtime for Auspex.
- `remote-connection-model.md` — detached/remote connection model.
- `session-source-model.md` and `session-source-implementation-notes.md` — session-source model and implementation notes.
- `mock-fixture-strategy.md` — mock/fallback fixture strategy.

### Operator and deployment

- `operator-security-tiers.md` — operator security tiers.
- `project-manifest-and-registry.md` — project manifest and registry notes.
- `brutus-deployment-status.md` — deployment status notes.

### Release process

- `auspex-release-framework.md` — release framework notes.
- `release-candidate-system.md` — release-candidate process notes.
- `release-coordination.md` — release coordination notes.

### Historical or planning-heavy docs

- `mvp-plan.md` — phased MVP path from backend seam to client shell; useful history, not automatically current.
- `app-skeleton-readiness.md` — shell readiness planning.
- `omegon-implementation-slice.md` — implementation-slice planning.

## Documentation hygiene

When refreshing a doc, add or update a short status note near the top:

- `Status: current implementation` — verified against source in this repository.
- `Status: design target` — intended direction, not fully implemented.
- `Status: historical` — retained for context; no longer authoritative.
- `Status: superseded by <doc>` — replaced by a newer source of truth.
