+++
title = "Policy-Gated Local Attach Probe"
tags = ["auspex","omegon","authorization","attach","probe"]
+++

+++
id = "0a2dd145-dfe0-4505-8c21-df112c92d530"
kind = "design_node"

[data]
title = "Policy-Gated Local Attach Probe"
status = "decided"
issue_type = "implementation-slice"
priority = 1
parent = "577cd4ab-2324-4e88-bf91-345083d53131"
dependencies = []
open_questions = []
+++

## Overview

# Policy-Gated Local Attach Probe

---
title: Policy-Gated Local Attach Probe
status: decided
tags: [auspex, omegon, authorization, attach, probe]
---

# Policy-Gated Local Attach Probe

Parent: [[native-local-management-mvp-implementation-plan]]

## Goal

Attach to a discovered local Omegon runtime in read/view mode only, while passing through the shared Auspex → Styrene policy adapter.

## Scope

This slice is read-only. It may fetch runtime metadata and state, then project it into the instance registry/COP. It must not launch, stop, restart, install packages, invoke HostActions, mutate runtime config, or send prompts.

## Authorization rule

Before probe/attach:

```text
principal + LocalOmegonAction::Attach + runtime resource + context -> PolicyDecision
```

For this slice, attach requires an operator identity/capability in the policy model. Until a real operator identity is wired, tests may use a synthetic principal. UI should not expose privileged attach controls without a valid principal.

## Probe sequence

1. Select a `LocalOmegonCandidate`.
2. Resolve `startup_url` from candidate or known defaults.
3. Fetch `/api/startup`.
4. Validate Omegon version/schema compatibility.
5. Resolve `state_url` from startup metadata.
6. Fetch `/api/state`.
7. Parse `OmegonStateSnapshot` through existing remote-session path.
8. Build/update `InstanceRecord` and projection.
9. Render attach result/evidence in COP.

## Safety decisions

- PID/port are locators, not identity.
- Runtime descriptor identity is the target resource identity.
- `AuspexOwned` does not bypass RBAC.
- Unreachable/incompatible candidates render evidence but do not become command targets.
- Tokens must not be printed in COP rows.

## Acceptance criteria

- `LocalOmegonProbeResult` exists.
- Read-only probe fetches startup/state for a local candidate.
- Successful probe can build an `AppController`/registry projection without mutation.
- Failure returns structured reason.
- Tests cover missing URL, startup parse failure, state parse failure, and policy-denied attach.

## Open Questions
