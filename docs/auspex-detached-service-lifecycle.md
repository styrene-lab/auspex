---
id: auspex-detached-service-lifecycle
title: "Auspex detached-service lifecycle and reattach semantics"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions: []
dependencies: []
related:
  - auspex-multi-agent-runtime
---

# Auspex detached-service lifecycle and reattach semantics

## Overview

Define ownership, persistence, shutdown, reattach, and abandonment behavior for long-running background Omegon workers that outlive an Auspex window or session.

## Decisions

### Detached-service workers remain registry-owned and are normally adopted by an Auspex background supervisor

**Status:** accepted

A detached worker may outlive a visible UI session, but it should not become an untracked orphan. Ownership should transfer from the UI session to a durable Auspex background supervisor/manager rather than disappear.

### Reattach authority is based on registry identity plus control-plane probe, not process existence alone

**Status:** accepted

A PID, pod name, or container id is insufficient for trusted reattach. Auspex should verify the registry record, control-plane identity, and readiness before treating a detached worker as reattached.

### Detached-service garbage collection is policy-driven with lost/abandoned state before reap

**Status:** accepted

Detached workers may temporarily disappear due to restart, eviction, or transient connectivity failure. They should not be reaped immediately on first failed probe.

## First-pass ownership states

- `session-owned` — launched and strongly owned by a visible Auspex session
- `daemon-owned` — adopted by an Auspex background supervisor
- `external` — discovered/attached rather than launched by this Auspex authority

## First-pass detached lifecycle state machine

### Runtime states

- `requested`
- `allocating`
- `starting`
- `ready`
- `busy`
- `degraded`
- `lost`
- `abandoned`
- `stopping`
- `exited`
- `reaped`

### Lifecycle transitions

```text
session-owned + continue-in-background
  -> daemon-owned

ready/busy + missed health/control-plane probe
  -> lost

lost + successful re-probe + identity match
  -> ready | busy | degraded

lost + TTL expired + no owner recovery
  -> abandoned

abandoned + explicit GC policy
  -> reaped
```

## First-pass reattach contract

To reattach a detached worker, Auspex should require:

1. matching `instance_id` in registry
2. matching expected control-plane schema / Omegon version bounds
3. successful probe of `/api/startup` and/or `/api/readyz`
4. usable auth/token reference resolution
5. backend placement still attributable to the same logical worker

Only then should the UI or supervisor mark the worker as reattached.

## First-pass shutdown semantics

Auspex should support three detached behaviors:

### 1. Stop on window close
- valid for session-owned workers only
- no adoption into daemon-owned state

### 2. Continue in background
- session-owned worker becomes daemon-owned
- registry remains authoritative
- worker stays reattachable later

### 3. Reap on expiry
- detached worker allowed to continue until TTL/policy expiry
- if still lost or abandoned at expiry, Auspex reaps it

## Backend-specific semantics

### Local detached
- usually managed by a local Auspex background supervisor
- `pid` may still be meaningful
- machine reboot can move worker directly to `exited` or `lost`

### OCI container
- supervisor should track container id + health
- restart policies may cause temporary `lost` followed by `ready`

### Kubernetes
- pod/job/deployment identity is not stable enough alone
- pod eviction or rescheduling should map to `lost` then reconcile back to `ready` if the logical worker is restored
- registry remains stable while observed placement changes

## Constraint

Detached-service workers are only worth supporting if they remain explainable and recoverable. If the system cannot tell the operator who owns a worker, whether it is still healthy, and whether it can be reattached, then detached mode is not production-safe.
