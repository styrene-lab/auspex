+++
title = "Auspex Authorization Recommendation"
tags = ["auspex","authorization","decision","cedar","styrene-rbac"]
+++

+++
id = "d4159cac-ea5a-494c-8bbe-671fdbf313a0"
kind = "design_node"

[data]
title = "Auspex Authorization Recommendation"
status = "decided"
issue_type = "decision"
priority = 1
parent = "4d700b75-2425-4182-8fa9-c0ff53b44293"
dependencies = []
open_questions = []
+++

## Overview

# Auspex Authorization Recommendation

---
title: Auspex Authorization Recommendation
status: decided
tags: [auspex, authorization, decision, cedar, styrene-rbac]
---

# Auspex Authorization Recommendation

Parent: [[authorization-substrate-evaluation-for-auspex]]

## Decision

Auspex should not grow an ad hoc identity → action → resource policy system in scattered Rust conditionals.

Use a layered model:

```text
styrene-identity
  -> principal identity and signing

styrene-rbac
  -> roster, role, coarse capability, signed hub grants

styrene-policy
  -> generic principal/action/resource/context request + decision substrate
  -> reasons, obligations, and supererogations
  -> supererogations are a superset-capable mirror of obligations

omegon-policy / auspex-policy
  -> domain-specific policy layers using styrene-policy primitives

Cedar-backed evaluator, after a simple native adapter spike
  -> policy-as-data for contextual authorization

Biscuit later
  -> delegated/attenuated grants for remote/browser/agent workflows
```

## Why

`styrene-rbac` is correct for principal capability resolution:

```rust
has_capability(identity_hash, capability) -> bool
```

But Auspex needs resource/context-sensitive decisions:

```text
Can operator A stop runtime R if ownership=AuspexOwned?
Can operator A attach to runtime R if compatibility=Unsupported?
Can operator A invoke package.install@1 if approval=false?
```

Those are not pure identity → capability checks. They include resource identity, ownership, compatibility, HostAction class, and approval state.

## Immediate implementation path

Before implementing attach/lifecycle controls, add an internal authorization model:

```rust
AuthorizationRequest {
    principal,
    action,
    resource,
    context,
}

AuthorizationDecision {
    effect,
    required_capability,
    requires_identity,
    requires_signature,
    requires_approval,
    audit_required,
    reasons,
}
```

The first evaluator can be a small Rust implementation in `styrene-policy` that maps actions to required capabilities and context gates. Then Cedar can be introduced behind the same trait if the spike confirms dependency/MSRV cost is acceptable. Auspex/Omegon-specific semantics should live in `auspex-policy` or `omegon-policy`, not in the generic substrate.

## Initial deny defaults

- Unknown operator identity blocks attach/control/lifecycle.
- User-owned/external stop is denied by default.
- Package install and mutating HostActions require approval and audit.
- `AuspexOwned` permits lifecycle consideration but does not bypass RBAC.
- Unsupported runtimes cannot be commanded.

## Candidate tooling result

| Tool | Decision |
|---|---|
| `styrene-rbac` | keep for identity role/capability substrate |
| Cedar | best next-level policy engine candidate |
| Biscuit | strong later candidate for delegated scoped grants |
| Oso | possible but less aligned with current identity/capability model |
| Casbin | viable but less semantically rich for principal/action/resource/context |
| Macaroons | too narrow for primary policy; possible delegation primitive only |

## Next code slice

Create:

```text
auspex-core/src/authorization.rs
```

with:

- `LocalOmegonAction`
- `AuthorizationResource`
- `AuthorizationContext`
- `AuthorizationRequest`
- `AuthorizationDecision`
- `AuthorizationEngine` trait
- `NativeAuthorizationEngine` simple evaluator

Acceptance tests:

- discovery allowed without identity
- attach denied without identity
- stop owned requires identity + owned lifecycle capability + approval
- stop external denied by default
- package install requires explicit grant + approval + audit
- unsupported runtime command denied

## Open Questions
