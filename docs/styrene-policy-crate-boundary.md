+++
title = "Styrene Policy Crate Boundary"
tags = ["styrene-policy","authorization","architecture","decision"]
+++

+++
id = "54a633b0-0853-445d-afd7-a60093d86b0a"
kind = "design_node"

[data]
title = "Styrene Policy Crate Boundary"
status = "decided"
issue_type = "decision"
priority = 1
parent = "4d700b75-2425-4182-8fa9-c0ff53b44293"
dependencies = []
open_questions = []
+++

## Overview

# Styrene Policy Crate Boundary

---
title: Styrene Policy Crate Boundary
status: decided
tags: [styrene-policy, authorization, architecture, decision]
---

# Styrene Policy Crate Boundary

Parent: [[authorization-substrate-evaluation-for-auspex]]

## Decision

Create a `styrene-policy` crate as the next authorization layer above `styrene-identity` and `styrene-rbac`.

Layering:

```text
styrene-identity
  -> principal identity, public keys, signing, attestations

styrene-rbac
  -> principal -> role/capability resolution

styrene-policy
  -> principal + action + resource + context -> decision

omegon-policy / auspex-policy / agent-specific policy
  -> domain policy using styrene-policy primitives
```

## Why

`styrene-rbac` should remain a small, auditable RBAC primitive:

```text
identity_hash -> role/capability
```

Auspex/Omegon/local-runtime control needs contextual authorization:

```text
principal + action + resource + context -> decision
```

Context includes ownership, target runtime identity, compatibility, approval state, HostAction class, package scope, secret scope, and audit requirements.

## Core API shape

```rust
pub trait PolicyEngine {
    fn authorize(&self, request: &PolicyRequest) -> PolicyDecision;
}

pub struct PolicyRequest {
    pub principal: PrincipalRef,
    pub action: ActionRef,
    pub resource: ResourceRef,
    pub context: PolicyContext,
}

pub struct PolicyDecision {
    pub effect: PolicyEffect,
    pub reasons: Vec<PolicyReason>,
    pub obligations: Vec<PolicyObligation>,
    pub supererogations: Vec<PolicySupererogation>,
}
```

## Effects

```rust
pub enum PolicyEffect {
    Allow,
    Deny,
}
```

## Obligations

Obligations are mandatory follow-up requirements attached to an authorization decision.

Examples:

```rust
pub enum PolicyObligation {
    RequireApproval,
    RequireAudit,
    RequireSignature,
    RequireFreshIdentity,
    RequireOwnershipProof,
    RequireRuntimeCompatibility,
}
```

Semantics:

```text
If the decision is allowed but obligations are present, the caller must satisfy/execute them before or during the action.
```

## Supererogations

Supererogations are recommended but non-mandatory actions that improve safety, operator comprehension, audit richness, or future recoverability.

Supererogations must be a **superset-capable mirror** of obligations: anything that can be mandatory should also be expressible as recommended. A policy may not require a signature, but the system should always be able to say that a signature would be appreciated.

Canonical shape:

```rust
pub enum PolicyFollowup {
    Approval,
    Audit,
    Signature,
    FreshIdentity,
    OwnershipProof,
    RuntimeCompatibility,
    AuditNote,
    OperatorConfirmation,
    RuntimeReprobe,
    CapabilityRefresh,
    DryRun,
    PostActionVerification,
    HumanReadableExplanation,
}

pub struct PolicyDecision {
    pub effect: PolicyEffect,
    pub reasons: Vec<PolicyReason>,
    pub obligations: Vec<PolicyFollowup>,
    pub supererogations: Vec<PolicyFollowup>,
}
```

Followups must be normalized before decisions are returned:

- each set is deduplicated
- obligations remove matching supererogations
- ordering is deterministic for audit stability
- future XOR/dominance relationships are declared centrally, not by caller convention

See [[policy-followup-normalization-exclusivity]].

Semantics:

```text
obligations      = must do before/during action
supererogations  = should do if cheap/available; never blocks execution
```

This lets policy say:

```text
Allowed, audit required, signature recommended, dry-run recommended.
```

without conflating hard requirements and good operational practice.

If supererogations become operational overhead, callers may drop or coalesce them internally. They should remain in the policy vocabulary so richer operators, audits, and future automation can use them.

## Domain extensions

`styrene-policy` should remain domain-neutral. Auspex/Omegon/agentic-work-specific semantics should live in extension crates or modules:

```text
omegon-policy
  HostAction, package install, tool invocation, runtime profile, agent delegation

auspex-policy
  local runtime discovery/attach/lifecycle, COP/operator approvals

agent-policy
  agentic work delegation, child agent boundaries, task/tool grants
```

These extensions should use `styrene-policy` primitives rather than invent parallel decision models.

## Backend strategy

Initial implementation:

```text
NativePolicyEngine
```

Then evaluate:

```text
CedarPolicyEngine
```

as a backend for policy-as-data.

Biscuit remains a strong later candidate for attenuated/delegated grants, especially for browser/server and agent-to-agent workflows.

## Consequences

- `styrene-rbac` remains simple and reusable.
- Auspex avoids hidden ad hoc policy conditionals.
- COP/audit can display reasons, obligations, and supererogations.
- Supererogations preserve optional safety affordances without turning them into hard gates.
- Domain-specific policy can grow without bloating the generic substrate.
- Browser/server mode can reuse the same request/decision shape later.

## Open Questions
