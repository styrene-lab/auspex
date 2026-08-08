---
id: auspex-omegon-runtime-control-plane
title: "Omegon runtime ownership and control plane"
status: exploring
parent: auspex-multi-agent-runtime
tags: [runtime, control-plane, profiles, workspaces, capabilities, menus]
open_questions:
  - "Which runtime resources are canonically owned by Auspex versus delegated to Omegon for validation and persistence?"
  - "Which mutations require operator review, explicit confirmation, or administrative authority?"
  - "How should Auspex negotiate typed ACP, IPC, WebSocket, HTTP, and local transports without treating slash commands as the primary API?"
  - "[assumption] Omegon's existing profile and capability schemas can be projected through stable management APIs without duplicating their semantics in Auspex."
  - "[assumption] A runtime launch can be represented by an immutable resolved specification with a stable digest."
dependencies:
  - auspex-worker-profiles
  - auspex-instance-registry-schema
related:
  - auspex-session-dispatcher
  - managed-agent-tool-interface
  - auspex-worker-inheritance
---

# Omegon runtime ownership and control plane

## Overview

Define the Auspex-owned operator control plane for the underlying Omegon runtime: profiles, workspaces, skills, extensions, permissions, launch configuration, running instances, and menu exposure.

This work extends beyond managed-agent interactions. Auspex owns resource catalogs, operator intent, approvals, launch provenance, lifecycle, and drift reconciliation. Omegon remains responsible for parsing and applying its native settings, loading capabilities, hosting tools, and reporting observed runtime state.

The control plane is resource-oriented. Slash commands, CLI commands, ACP, IPC, WebSocket, HTTP, model tools, and extension RPC are transport bindings rather than separate information architectures.

## Decisions

### Resource ownership precedes menu implementation

**Status:** accepted

Menus will project canonical resources and actions. They will not be assembled directly from slash commands or model tool definitions.

### Launch consumes a resolved profile; it does not implicitly create one

**Status:** accepted

Profile creation and editing are explicit persistent operations. Launch resolves a selected profile, workspace, capability policy, and bounded overrides into an immutable launch specification.

### Intended and observed runtime state remain distinct

**Status:** accepted

Profiles and launch specifications describe intent. Runtime projections describe observed state. Auspex reports drift and offers explicit apply, reload, restart, or save operations rather than silently reconciling differences.

### Privileged administration is operator-facing

**Status:** accepted

Installing executable extensions, broadening filesystem trust, changing credentials, and expanding permissions are operator control-plane operations. Model-visible tools may request these changes but do not gain implicit authority to perform them.

## Child work

- [[auspex-omegon-control-surface-catalog]] — canonical action/resource matrix and transport bindings.
- [[auspex-runtime-profile-registry]] — profile ownership, editing, revisions, and effective policy.
- [[auspex-workspace-registry]] — working directories, repository identity, trust, and mutability.
- [[auspex-capability-lifecycle]] — skills, extensions, plugins, packages, installation, activation, and loading.
- [[auspex-runtime-launch-spec]] — immutable launch resolution and provenance.
- [[auspex-runtime-reconciliation]] — observed state, drift, reload/restart, and runtime lifecycle.
- [[auspex-runtime-menu-projection]] — renderer-neutral menu taxonomy and action metadata.

## Implementation Notes

### Constraints

- Preserve Auspex as supervisor and lifecycle authority.
- Reuse Omegon-native profile and capability semantics rather than inventing incompatible duplicates.
- Prefer typed transports; slash execution is a compatibility fallback only.
- Every mutation declares authority, persistence scope, runtime effect, confirmation policy, and side effects.
- Paths are canonicalized and trust grants remain distinct from workspace registration.
- Installed, permitted, activated, and loaded capability states are represented independently.
