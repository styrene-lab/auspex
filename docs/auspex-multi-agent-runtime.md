---
id: auspex-multi-agent-runtime
title: "Auspex multi-agent runtime and Omegon instance orchestration"
status: exploring
tags: []
open_questions:
  - "Should detached-service workers remain owned by an Auspex background supervisor, or be allowed to drift into weaker external ownership after launch?"
  - "How much auth, memory, and workspace state should supervised-child workers inherit by default from the primary driver?"
dependencies: []
related:
  - auspex-instance-registry-schema
  - auspex-worker-profiles
  - auspex-runtime-backends
  - auspex-detached-service-lifecycle
---

# Auspex multi-agent runtime and Omegon instance orchestration

## Overview

Design how Auspex should supervise multiple Omegon instances, long-lived background agents, and service-style agent hosting beyond the single embedded primary driver.

The near-term goal is **not** to turn Auspex into the agent runtime itself.
The near-term goal is to make Auspex the **supervisor/gateway** for a pool of Omegon workers.

## Research

### Existing Auspex runtime doctrine already favors subprocess supervision

`docs/embedded-runtime-model.md` already makes a strong near-term architectural choice: Auspex should be the sovereign product shell, while Omegon remains a specialized embedded subsystem. It explicitly prefers **Option A — separate managed subprocesses** for the first implementation path because it preserves fault isolation, reuses existing binaries, avoids premature library coupling, and makes supervision explicit.

This argues against jumping straight to “Auspex becomes the agent runtime itself” for the first multi-agent implementation.

### Omegon already has a reusable per-instance control-plane contract

Omegon 0.15.7 exposes a reusable embedded control-plane contract via `omegon embedded`.
Each instance can bind its own localhost port, emit startup JSON, expose `/api/startup`, `/api/state`, `/api/healthz`, `/api/readyz`, and `/ws?token=...`, and identify itself with schema version + auth token.

This is enough to treat **each Omegon process as a manageable worker instance** behind Auspex without inventing a new transport protocol first.

### External orchestration patterns point to a supervisor/gateway layer

Research examples from OpenClaw and Claude Cowork-style multi-agent systems converge on a common pattern:

- a **supervisor/gateway layer** tracks agent identity, workspace, lifecycle, and routing
- worker agents remain isolated runtimes/processes
- orchestration state lives above the workers, not inside one of them

This supports making Auspex the orchestration/supervision layer while keeping Omegon instances as workers.

### Long-running background agents require durable re-attach metadata

If Auspex should be able to close/reopen while background Omegon agents continue running, then each instance needs durable metadata outside window memory:

- stable instance id
- launch command / binary version
- port / base URL
- auth token or token recovery mechanism
- cwd / workspace owner
- role (primary, child, detached service, relay worker)
- last known lifecycle state
- pid and exit status if supervised locally

That implies an **instance registry** owned by Auspex.

## Decisions

### Near-term multi-agent model is a supervised Omegon instance pool, not in-process agent embedding

**Status:** accepted

Auspex should own orchestration and supervision, while each Omegon remains an isolated worker process exposing the existing embedded control-plane contract.

### Introduce an Auspex-owned instance registry for primary, child, and detached Omegon agents

**Status:** accepted

Multiple agents and background persistence require durable identity and re-attach metadata. A registry lets Auspex explain, reconnect to, and clean up workers instead of losing track of them.

### Support three instance roles: primary-driver, supervised-child, and detached-service

**Status:** accepted

These roles cover the immediate product needs without over-generalizing.

- `primary-driver` powers the main UI
- `supervised-child` handles delegated parallel work
- `detached-service` supports long-running background work that survives window/session restarts

### Auspex-as-agent/OpenClaw-style gateway remains a later-stage option, not the first path

**Status:** accepted

Making Auspex the agent runtime itself would couple UI and orchestration too tightly before the multi-instance supervision model is proven.

### Treat an Omegon instance as a logical worker, not merely a local process

**Status:** accepted

The registry and orchestration model must support multiple execution backends:

- local subprocess
- detached local service
- OCI/containerized worker
- Kubernetes-managed worker

That means worker identity, lifecycle, and policy must be backend-agnostic.

### Supervisor workers use stronger models; delegated workers default to cheaper models

**Status:** accepted

Profiled worker classes are required.
The supervisor path should prefer stronger models and higher thinking budgets.
Child/subtask workers should default to cheaper models such as Haiku, GPT-spark, or equivalent low-cost/local options.
Escalation to expensive models should be explicit rather than the default.

### Kubernetes is a first-class runtime backend, not an afterthought

**Status:** accepted

Auspex should be design-compatible with running as a deployable service in Kubernetes, with Omegon workers instantiated as pod/job/deployment-backed runtimes under the same logical worker model.

## First-pass runtime model

Auspex supervises a pool of logical workers.
Each worker has:

1. **Identity** — stable, durable id and role
2. **Placement** — how/where it runs
3. **Control plane** — how Auspex talks to it
4. **Policy** — which profile/knobs it gets
5. **Task binding** — why it exists and who owns it

## First-pass worker lifecycle

A worker instance should move through:

- `requested`
- `allocating`
- `starting`
- `ready`
- `busy`
- `degraded`
- `stopping`
- `exited`
- `lost`

This lifecycle must work for both local and Kubernetes-backed workers.

## First-pass child design split

### [[auspex-instance-registry-schema]]
Owns the persistent schema for worker identity, desired state, observed state, and control-plane references.

### [[auspex-worker-profiles]]
Owns profile definitions and knob allocation rules for supervisor, primary, child, and detached workers.

### [[auspex-runtime-backends]]
Owns the normalized instantiation/reconciliation contract across local, OCI, and Kubernetes backends.

### [[auspex-detached-service-lifecycle]]
Owns the semantics of persistence, reattach, shutdown, abandonment, and cleanup for background agents.

## Open Questions

- Should detached-service workers remain owned by an Auspex background supervisor, or be allowed to drift into weaker external ownership after launch?
- How much auth, memory, and workspace state should supervised-child workers inherit by default from the primary driver?
