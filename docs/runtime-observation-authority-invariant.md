+++
title = "Runtime Observation vs Authority Invariant"
tags = ["auspex","runtime","authority","observation","invariant"]
+++

+++
id = "0295515b-4c51-4e93-a775-1eed5cd71003"
kind = "design_node"

[data]
title = "Runtime Observation vs Authority Invariant"
status = "decided"
issue_type = "decision"
priority = 1
parent = "e815b23d-0986-4e4f-b143-f89e44f80432"
dependencies = []
open_questions = []
+++

## Overview

# Runtime Observation vs Authority Invariant

---
title: Runtime Observation vs Authority Invariant
status: decided
tags: [auspex, runtime, authority, observation, invariant]
---

# Runtime Observation vs Authority Invariant

Parent: [[local-omegon-instance-management-mvp]]

## Decision

Auspex must not use “attached runtime” as a synonym for authority.

Current native MVP state is:

```text
Auspex observes local Omegon runtimes.
Auspex can select observed runtimes for inspection.
Auspex escalates to command/lifecycle authority only through explicit policy, freshness, ownership, and credential gates.
```

Future embedded mode is separate:

```text
Auspex may later spawn an AuspexOwned central Omegon runtime for its own orchestration loop.
That is not assumed in the local observation MVP.
```

## State axes

```rust
enum RuntimeObservationState {
    Discovered,
    ProbedFresh,
    PersistedStale,
    Lost,
}

enum RuntimeAuthorityState {
    None,
    ReadOnly,
    CommandCandidate,
    CommandAuthorized,
    LifecycleCandidate,
    LifecycleAuthorized,
}

enum RuntimeOwnershipState {
    AuspexOwned,
    OperatorOwned,
    External,
    Unknown,
}
```

For the current observed `web-compat` runtime:

```text
observation = ProbedFresh
authority   = ReadOnly
ownership   = OperatorOwned or External
```

For a future embedded Auspex runtime:

```text
observation = ProbedFresh
authority   = CommandAuthorized / LifecycleAuthorized
ownership   = AuspexOwned
```

## Terminology rules

Use now:

```text
Observed Runtime
Runtime Probe
Observed Fleet
Read-only Projection
Fresh/Stale Observation
```

Reserve for later:

```text
Auspex Embedded Runtime
Central Runtime
AuspexOwned Runtime
Coordinator Runtime
Spawned Runtime
```

Avoid in the current MVP unless authority exists:

```text
Attached Runtime
Embedded Runtime
Lifecycle Managed
```

## Immediate UI/COP replacements

| Current | Replace with |
|---|---|
| Attach Probe | Probe Runtime |
| Local Omegon Attach | Local Omegon Probe |
| Attached Fleet | Observed Runtimes |
| attached read-only projection | observed read-only projection |
| Primary Coordinator | Observed Primary Runtime |
| Deployment | Runtime Observations |
| live | fresh |
| Attached to Omegon instance | Observing Omegon runtime |

## Persistence implication

Persisted probe records are persisted observation evidence, not persisted authority.

On startup:

```text
ObservationState = PersistedStale
AuthorityState   = None / ReadOnlyDeniedUntilReprobe
OwnershipState   = prior observed ownership, not authority
```

Fresh reprobe may promote:

```text
PersistedStale → ProbedFresh
None → ReadOnly
```

Only policy + command transport may promote:

```text
ReadOnly → CommandAuthorized
```

Only AuspexOwned + lifecycle handle may promote:

```text
CommandAuthorized → LifecycleAuthorized
```

## Open Questions
