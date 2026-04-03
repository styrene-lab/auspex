---
id: auspex-detached-service-lifecycle
title: "Auspex detached-service lifecycle and reattach semantics"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions:
  - "Should detached-service workers remain owned by an Auspex background supervisor, or be allowed to weaken into externally re-attachable services?"
dependencies: []
related:
  - auspex-multi-agent-runtime
---

# Auspex detached-service lifecycle and reattach semantics

## Overview

Define ownership, persistence, shutdown, reattach, and abandonment behavior for long-running background Omegon workers that outlive an Auspex window or session.

## First-pass decision direction

### Detached workers should remain registry-owned even if UI ownership changes

**Status:** proposed

A detached worker may outlive a window, but it should not become an untracked orphan. The registry must continue to represent ownership and reattach semantics even after the visible UI session ends.

## First-pass lifecycle concerns

Detached-service workers need explicit handling for:

- launch ownership
- reattach after Auspex restart
- graceful shutdown
- lost worker detection
- garbage collection of abandoned workers
- token/secret recovery

## First-pass model

### Ownership states

- `session-owned` — launched and strongly owned by a visible Auspex session
- `daemon-owned` — adopted by a background Auspex supervisor
- `external` — discovered/attached rather than launched

### Reattach expectations

Auspex should be able to:

- list detached workers from registry
- probe their control planes
- mark them `ready`, `degraded`, `exited`, or `lost`
- reattach the UI without recreating them

### Shutdown semantics

Auspex should support:

- `stop when window closes` for session-owned workers
- `continue in background` for detached-service workers
- `reap abandoned instances after TTL/policy` when they are lost or explicitly expired

## Constraint

Detached-service workers are only worth supporting if they remain explainable and recoverable. If reattach metadata or secret recovery is weak, the feature becomes an orphan-process generator.
