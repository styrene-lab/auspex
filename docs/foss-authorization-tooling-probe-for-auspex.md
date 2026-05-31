+++
title = "FOSS Authorization Tooling Probe for Auspex"
tags = ["auspex","authorization","cedar","oso","biscuit","casbin","research"]
+++

+++
id = "1e3c892c-edfb-4fe3-b977-0f7e2e6c6413"
kind = "design_node"

[data]
title = "FOSS Authorization Tooling Probe for Auspex"
status = "exploring"
issue_type = "research"
priority = 1
parent = "4d700b75-2425-4182-8fa9-c0ff53b44293"
dependencies = []
open_questions = []
+++

## Overview

# FOSS Authorization Tooling Probe for Auspex

---
title: FOSS Authorization Tooling Probe for Auspex
status: exploring
tags: [auspex, authorization, cedar, oso, biscuit, casbin, research]
---

# FOSS Authorization Tooling Probe for Auspex

Parent: [[authorization-substrate-evaluation-for-auspex]]

## Crates probed

```text
cedar-policy = 4.11.0
oso = 0.27.3
biscuit-auth = 6.0.0
casbin = 2.20.0
macaroon = 0.3.0
```

## Candidate comparison

| Tool | Fit | Strengths | Costs / concerns |
|---|---|---|---|
| Cedar | strong | Rust-native, explicit principal/action/resource/context, explainable decisions, schemas, offline evaluation, AWS-proven model | Rust 1.89+ requirement; policy/schema complexity |
| Oso | medium | Expressive object/resource rules, embedded app authorization, Rust crate | Less aligned with signed/distributed capability story; Polar runtime semantics are another language |
| Biscuit | medium/strong for delegation | Offline attenuated authorization tokens, Datalog, wasm support, good for delegating scoped grants | Token/delegation layer, not a complete local policy engine by itself |
| Casbin | medium | Mature ACL/RBAC/ABAC models, Rust crate, simple operational model | Model strings/config; less rich entity/context semantics than Cedar; async/runtime baggage |
| Macaroons | narrow | Caveated bearer delegation | Bearer-token semantics; weaker fit for primary local operator authorization |

## Cedar-specific fit

Cedar's conceptual model directly matches Auspex's problem:

```text
principal: Operator::"identity_hash"
action: Action::"auspex.instance.stop_owned"
resource: Runtime::"instance_id"
context: {
  ownership: "AuspexOwned",
  compatibility: "Compatible",
  approval: true,
  host_action_class: "MutatingRequiresApproval"
}
```

This is exactly the semantic layer missing from `styrene-rbac`.

Cedar can represent:

- deny-by-default
- explicit action/resource grants
- runtime ownership conditions
- approval-required context
- schema validation
- explainable deny reasons for COP/audit
- offline thick-client evaluation

## Biscuit-specific fit

Biscuit is not the first policy engine I would use for the local MVP, but it is attractive later for delegated/scoped grants:

```text
Allow this agent to attach to runtime X for 30 minutes.
Allow this child to invoke only read-only HostActions.
Allow package install for package Y once.
```

It fits future server/client and agent-to-agent delegation better than immediate local decisioning.

## Recommended cake slice

Do not replace `styrene-rbac` yet.

Recommended layering:

```text
styrene-identity
  -> establishes operator/runtime public identity

styrene-rbac
  -> resolves baseline role + coarse capabilities from roster/signed hub entries

Auspex policy adapter
  -> converts local action/resource/context into authorization request

Cedar policy engine
  -> evaluates resource/context-sensitive authorization

Biscuit later
  -> delegated/attenuated grants for remote/browser/agent-to-agent workflows
```

## Why not only styrene-rbac?

Extending `styrene-rbac` with strings like `auspex.instance.stop_owned` is necessary but not sufficient. The hard part is contextual:

```text
same operator + same capability + different runtime ownership = different decision
```

If we implement that ad hoc in Rust conditionals, Auspex grows a hidden policy engine without schemas, explainability, or reusable tests.

## Recommendation

Prototype a small Cedar-backed authorization adapter in Auspex before implementing attach/lifecycle controls.

Keep it behind an internal interface so we can still fall back to a simple native Rust policy if Cedar's dependency or MSRV cost is too high.

```rust
trait AuthorizationEngine {
    fn authorize(&self, request: AuthorizationRequest) -> AuthorizationDecision;
}
```

First implementation can be Rust-native/simple. Second implementation can be Cedar. The request/decision model is the important cake-slice boundary.

## Open Questions
