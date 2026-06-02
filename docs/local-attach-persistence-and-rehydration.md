+++
title = "Local Attach Persistence and Rehydration"
tags = ["auspex","native","local-management","mvp","persistence"]
+++

+++
id = "a2c3d59f-830f-465c-a31d-55fe29c7c25b"
kind = "design_node"

[data]
title = "Local Attach Persistence and Rehydration"
status = "decided"
issue_type = "plan"
priority = 1
parent = "e815b23d-0986-4e4f-b143-f89e44f80432"
dependencies = []
open_questions = []
+++

## Overview

# Local Attach Persistence and Rehydration

---
title: Local Attach Persistence and Rehydration
status: decided
tags: [auspex, native, local-management, mvp, persistence]
---

# Local Attach Persistence and Rehydration

Parent: [[local-omegon-instance-management-mvp]]

## Goal

Make native Auspex remember a successfully attached local Omegon runtime across process restarts while preserving the safety distinction between:

```text
persisted evidence
freshly probed authority
```

The current attach path proves read-only local attach works, but it is mostly session-ephemeral. The MVP needs durable runtime management substrate.

## Problem

Current flow:

```text
Discover Local
→ Attach Probe
→ in-memory registry/projection/chrome state
```

After app restart, the operator must attach again before the UI knows about the local runtime.

That is acceptable for a spike, not for MVP runtime management.

## Decision

Persist successful local attach records into the existing instance registry store, then rehydrate them on native app startup.

Rehydrated records are treated as **stale/read-only evidence** until a fresh probe confirms liveness, compatibility, auth, and policy allowance.

## Safety model

Persistence does **not** imply command authority.

A persisted runtime record may answer:

```text
Auspex previously saw this runtime.
Here is its last known descriptor/control-plane/capability evidence.
```

It may not answer:

```text
This runtime is currently alive.
This token is still valid.
This operator may command it.
This runtime may be stopped/restarted.
```

Those require a fresh probe and policy decision.

## Persisted data

After successful `Attach Probe`, persist an `InstanceRecord` with:

- instance id
- role/profile interpreted values
- raw role/profile/runtime profile
- ownership
- placement PID/CWD/host
- control-plane URLs:
  - base
  - startup
  - state
  - health
  - ready
  - websocket
  - ACP, when available
- auth mode/source metadata when available
- compatibility assessment
- capability snapshot
- derived operational profile
- last observed freshness
- last attach/probe evidence marker

## Startup behavior

On native app startup:

1. Load the instance registry.
2. Rehydrate persisted local records into the attached-instance engine.
3. Mark rehydrated local records stale/unknown until reprobed.
4. Populate:
   - left rail
   - top rail
   - deployment count
   - fleet projection
5. Do not select a stale runtime as command-authoritative unless explicitly allowed by policy.

## Refresh behavior

`Refresh Fleet` reads current registry/projection state and should show persisted records.

`Attach Probe` updates the existing record by `instance_id` or stable local runtime identity instead of appending duplicates.

## Stable identity

Record matching priority:

1. `instance_id`
2. `state_url`
3. `startup_url`
4. `pid` only when still alive and matching command evidence

PID alone is not stable across restarts.

## Acceptance criteria

### 1. Attach persists registry record

Given a successful local attach probe
When the probe completes
Then the instance registry contains the attached runtime record
And the record includes compatibility, capabilities, raw descriptor fields, and derived operational profile.

### 2. Startup rehydrates persisted runtime

Given a persisted local runtime record
When native Auspex starts
Then the runtime appears in fleet/deployment projection
And the deployment summary counts it.

### 3. Rehydrated runtime is stale until reprobed

Given a persisted local runtime record from a previous process
When native Auspex starts
Then the record is marked stale/unknown freshness
And it is not treated as freshly commandable.

### 4. Attach probe updates existing record

Given a persisted local runtime record
When Attach Probe succeeds for the same runtime
Then the existing record is updated
And no duplicate instance row is created.

### 5. Browser mode does not gain host authority

Given wasm/browser mode
When the app renders the COP
Then persisted local host probing/lifecycle authority remains unavailable.

## Implementation plan

### Step 1 — registry persistence hook

Update `probe_first_local_omegon_to_cop()` so successful local attach writes the updated registry through the existing persistence path.

Files:

```text
auspex-core/src/controller.rs
auspex-core/src/instance_registry.rs
```

### Step 2 — stale rehydration model

Add or reuse a method that marks persisted local records stale during startup rehydration unless a fresh probe occurred in this process.

Potential fields:

```rust
ObservedHealth {
    ready: false or last-known-ready,
    freshness: Some(Stale),
    degraded_reason: Some("requires fresh local probe"),
}
```

### Step 3 — duplicate prevention

Ensure local attach updates by stable key:

```text
instance_id OR state_url OR startup_url
```

not raw append.

### Step 4 — tests

Add tests:

```text
attach_probe_persists_registry_record
startup_rehydrates_persisted_local_runtime
rehydrated_runtime_is_stale_until_reprobed
attach_probe_updates_existing_record_without_duplicate
```

### Step 5 — COP copy

Expose distinction in COP:

```text
fresh attached
stale persisted
needs reprobe
```

## Non-goals

- No stop/restart controls in this slice.
- No mutating HostAction invocation.
- No browser/server host authority bridge.
- No replacement of dev principal with real Styrene identity yet.

## Follow-on slices

1. Freshness/reprobe model refinements.
2. Real Styrene identity/RBAC principal.
3. AuspexOwned lifecycle controls.
4. Evidence/project-rules COP panel for 0.26 substrate.

## Open Questions

## Dogfood validation hook

Persistence should be designed so later Nex sandbox runtimes use the same path:

```text
sandbox launch evidence
→ AuspexOwned InstanceRecord
→ persisted stale record
→ fresh reprobe required before command/lifecycle authority
```

This lets [[native-auspex-nex-sandbox-dogfood-lane]] validate that sandbox-created Omegon runtimes can survive Auspex restarts without granting stale authority.
