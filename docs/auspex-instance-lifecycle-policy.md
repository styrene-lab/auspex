---
id: auspex-instance-lifecycle-policy
title: "Attached Omegon instance lifecycle policy"
status: resolved
parent: auspex-embedded-operator-parity
tags: [state-engine, lifecycle, instance-registry]
open_questions:
  - "What freshness model should Auspex use for attached instances: event-driven only, heartbeat timestamps, or hybrid last-seen plus control-plane readiness checks?"
  - "When should Auspex detach or archive an instance record automatically versus marking it stale for operator review?"
  - "How should ownership drift be resolved when a persisted registry record conflicts with live session/session-key observations?"
  - "What cleanup policy should apply differently to primary dispatcher instances, supervised child instances, and detached services?"
dependencies: []
related: []
---

# Attached Omegon instance lifecycle policy

## Overview

Define the durable lifecycle policy for attached Omegon instances in Auspex: freshness, last-seen semantics, stale expiry, detach rules, ownership drift, and cleanup behavior as registry-backed multi-instance supervision evolves.

## Research

### Lifecycle policy synthesis

Grounded by prior detached-service decisions: detached workers remain registry-owned and normally supervisor-adopted; reattach authority is registry identity plus control-plane probe; garbage collection is policy-driven through lost/abandoned before reap. For attached instances in Auspex, this implies a hybrid freshness model (last-seen timestamps plus control-plane evidence), live ownership taking precedence over stale persisted ownership, and role-specific cleanup windows for primary dispatcher, supervised child, and detached-service roles.

## Decisions

### Use hybrid freshness with last-seen plus control-plane evidence

**Status:** accepted

**Rationale:** Event-only freshness is too brittle for reconnecting workers and detached services, while probe-only freshness loses recency semantics. Auspex should maintain per-instance last-seen timestamps from live events/snapshots and combine them with control-plane readiness evidence when available.

### Distinguish stale from detached and archive only after policy expiry

**Status:** accepted

**Rationale:** Automatic archive/reap on first disappearance is unsafe. Session-owned host/dispatcher instances may be detached or purged when live session authority disappears, but detached-service records should transition through stale/lost or abandoned policy states before archive or reap.

### Live ownership evidence overrides persisted session ownership

**Status:** accepted

**Rationale:** Persisted registry ownership can drift after reconnects or adoption. When a live session descriptor, dispatcher binding, or control-plane identity proves current ownership, Auspex should treat persisted ownership as stale and update the registry rather than preserving the old owner.

### Apply differentiated cleanup by instance role

**Status:** accepted

**Rationale:** Primary dispatcher instances are operator-critical and should be reconciled aggressively; supervised children may be purged when absent from the current session set; detached services should follow the longer-lived detached-service policy with stale/lost/abandoned phases before reap.

## Open Questions

- What freshness model should Auspex use for attached instances: event-driven only, heartbeat timestamps, or hybrid last-seen plus control-plane readiness checks?
- When should Auspex detach or archive an instance record automatically versus marking it stale for operator review?
- How should ownership drift be resolved when a persisted registry record conflicts with live session/session-key observations?
- What cleanup policy should apply differently to primary dispatcher instances, supervised child instances, and detached services?
