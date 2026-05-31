+++
title = "Authorization Substrate Evaluation for Auspex"
tags = ["auspex","authorization","rbac","cedar","oso","biscuit","casbin"]
+++

+++
id = "4d700b75-2425-4182-8fa9-c0ff53b44293"
kind = "design_node"

[data]
title = "Authorization Substrate Evaluation for Auspex"
status = "exploring"
issue_type = "research"
priority = 1
parent = "e56e85e7-f632-4c3c-994f-a16c9d713ac0"
dependencies = []
open_questions = []
+++

## Overview

# Authorization Substrate Evaluation for Auspex

---
title: Authorization Substrate Evaluation for Auspex
status: exploring
tags: [auspex, authorization, rbac, cedar, oso, biscuit, casbin]
---

# Authorization Substrate Evaluation for Auspex

Parent: [[native-local-management-identity-rbac-gates]]

## Question

Should Auspex extend `styrene-rbac`, or should it adopt a next-layer authorization substrate so we do not hand-wire identity → action → resource semantics?

## Scope

Evaluate the authorization substrate for Auspex local and fleet actions:

```text
operator identity + runtime identity + resource + action + context → decision
```

Context includes ownership (`AuspexOwned`), compatibility, capability evidence, approvals, and audit requirements.

## Candidate classes

1. Extend `styrene-rbac` capability strings and roles.
2. Use a policy engine such as Cedar.
3. Use relationship/attribute authorization such as Oso Polar.
4. Use distributed authorization tokens such as Biscuit.
5. Use ACL/RBAC/ABAC library such as Casbin.
6. Use capability grants/macaroons for scoped delegation.

## Evaluation criteria

- Rust-native or FFI-free if possible.
- Can represent action/resource/context, not only identity capability strings.
- Can express deny-by-default and explicit approval gates.
- Can represent resource ownership and target runtime identity.
- Can support offline/local decisions.
- Can emit explainable decisions for COP/audit.
- Does not force a server dependency for thick-client MVP.
- Can coexist with Styrene identity hashes and signed roster entries.
- Can later support browser/server split.

## Known current substrate

`styrene-rbac` provides:

- `Role`: `Blocked`, `None`, `Peer`, `Monitor`, `Operator`, `Admin`
- `RbacPolicy`
- `RosterEntry`
- explicit grants
- blocked identity prefixes
- trusted hubs and signed roster entries
- pure `has_capability(identity_hash, capability)` evaluation

This is strong for identity → role/capability, but weak for resource/context-sensitive authorization unless Auspex hand-wires semantic checks around it.

## Output expected

A recommendation for Auspex's next authorization layer:

- Keep `styrene-rbac` only.
- Extend `styrene-rbac` with resource-aware policy.
- Adopt a policy engine.
- Hybrid: Styrene identity/RBAC for principal resolution + policy engine for action/resource/context.

## Open Questions
