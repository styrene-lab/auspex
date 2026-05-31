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

Examples:

```rust
pub enum PolicySupererogation {
    RecommendAuditNote,
    RecommendOperatorConfirmation,
    RecommendRuntimeReprobe,
    RecommendCapabilityRefresh,
    RecommendDryRun,
    RecommendPostActionVerification,
    RecommendHumanReadableExplanation,
}
```

Semantics:

```text
Supererogations do not block action execution. They are advisory improvements surfaced to the caller/COP/operator.
```

This lets policy say:

```text
Allowed, audit required, and a dry-run is recommended.
```

without conflating hard requirements and good operational practice.

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
- Domain-specific policy can grow without bloating the generic substrate.
- Browser/server mode can reuse the same request/decision shape later.

## Open Questions
