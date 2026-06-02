+++
title = "Native Auspex Nex Sandbox Dogfood Lane"
tags = ["auspex","nex","omegon","sandbox","dogfood","validation"]
+++

+++
id = "336a7c11-f0dc-4492-b98a-f63ec38a3db5"
kind = "design_node"

[data]
title = "Native Auspex Nex Sandbox Dogfood Lane"
status = "decided"
issue_type = "plan"
priority = 1
parent = "e815b23d-0986-4e4f-b143-f89e44f80432"
dependencies = []
open_questions = []
+++

## Overview

# Native Auspex Nex Sandbox Dogfood Lane

---
title: Native Auspex Nex Sandbox Dogfood Lane
status: decided
tags: [auspex, nex, omegon, sandbox, dogfood, validation]
---

# Native Auspex Nex Sandbox Dogfood Lane

Parent: [[local-omegon-instance-management-mvp]]

## Goal

Use native Auspex to validate that our own ecosystem can launch and supervise constrained Omegon sandboxes through Nex-backed substrate/policy capabilities.

This is a dogfood lane, not a generic cloud orchestration system. The first target is our own local development loop:

```text
native Auspex
→ request simple Nex sandbox
→ launch constrained Omegon runtime inside/through that sandbox
→ attach read-only
→ validate limited credentials/capabilities
→ collect evidence/project-rules output
→ iterate toward richer ecosystem sandboxes
```

## Why this matters

Omegon 0.26 introduced evidence, project-rules, and Nex substrate read-only boundaries. Auspex is now beginning to model local Omegon runtimes. The next validation layer is to prove Auspex can use those substrate capabilities to create safer, reproducible local sandboxes for Omegon itself.

This gives us a concrete self-hosted test path for:

- Nex substrate inspection
- limited credential grants
- local runtime isolation
- evidence map generation
- project-rules advisory checks
- local attach/probe behavior
- lifecycle controls once AuspexOwned ownership is established

## Initial sandbox shape

The first sandbox should be intentionally small:

```text
name: omegon-dev-smoke
owner: native-auspex
runtime: local process or devenv-backed shell
credentials: limited / no write secrets by default
workspace: current Auspex repo or temp worktree
allowed network: localhost + configured package/index endpoints only
allowed tools: omegon, nex, cargo/check/test as explicitly declared
output: startup/state endpoints + .omegon/evidence substrate
```

## Capability boundary

Auspex must distinguish:

```text
read-only substrate inspection
sandbox launch request
sandboxed Omegon attach/probe
sandbox lifecycle mutation
credential grant mutation
```

Only the first two are candidates for early MVP. Lifecycle and credential mutation require stronger identity/RBAC/policy gates.

## Limited credentials model

The first dogfood credential model should support:

- no secrets by default
- explicit allowlist of credential references
- read-only package/index credentials where needed
- no ambient host credential inheritance
- credential grants visible in COP/audit
- grant expiry or per-sandbox scope

Example future policy request:

```text
principal: local operator
resource: sandbox omegon-dev-smoke
action: sandbox.launch
context:
  credential_grants: ["secret://nex/package-index/read-only"]
  network_scope: "localhost+declared-indexes"
  workspace_scope: "temp-worktree"
  evidence_required: true
```

## Dogfood validation path

### Phase 1 — inspect only

Use Auspex to call/read Nex substrate reports and render:

- devenv availability
- machine profile
- sandbox prerequisites
- missing tools/packages
- advisory degradation when Nex is absent

No sandbox launch yet.

### Phase 2 — simple sandbox launch plan

Auspex builds a launch plan but does not execute it:

```text
sandbox id
workspace path
env allowlist
credential grants
expected Omegon command
expected evidence outputs
expected cleanup action
```

### Phase 3 — launch constrained Omegon sandbox

Native Auspex launches a simple sandboxed Omegon runtime with limited credentials and records:

- PID/process identity
- ownership = AuspexOwned
- startup/state URLs
- sandbox policy summary
- evidence directory
- cleanup handle

### Phase 4 — attach/probe and evidence validation

Auspex attaches read-only to the sandboxed runtime and validates:

- compatibility
- operational profile
- capability snapshot
- evidence substrate presence
- project-rules result, advisory only at first

### Phase 5 — lifecycle dogfood

Only after identity/policy gates are in place:

- stop owned sandbox
- restart owned sandbox
- collect audit/evidence of lifecycle operation

## Acceptance criteria

### Inspect-only MVP

- Native Auspex can show Nex substrate status in COP.
- Missing Nex is advisory/degraded, not fatal.
- Substrate reports are read-only and workspace-bound.

### Plan-only MVP

- Auspex can render a sandbox launch plan without executing it.
- Plan includes credential, network, workspace, command, evidence, and cleanup sections.
- Policy decision is shown before launch.

### Launch MVP

- Auspex can launch one simple constrained Omegon sandbox.
- The sandbox does not inherit ambient secrets.
- Attach Probe sees the sandboxed runtime as compatible.
- Registry marks it `AuspexOwned` only when Auspex launched it.
- Evidence/project-rules outputs are visible in COP.

## Non-goals

- No Kubernetes/remote cluster orchestration in this lane.
- No generic multi-tenant sandbox product.
- No lifecycle mutation for user-owned Omegon runtimes.
- No automatic secret grant expansion.
- No browser/wasm host authority.

## Future code areas

Likely future files/modules:

```text
auspex-core/src/nex_substrate.rs
auspex-core/src/sandbox_plan.rs
auspex-core/src/sandbox_launch.rs
auspex-core/src/credential_grants.rs
auspex-core/src/evidence_projection.rs
```

Existing modules to integrate:

```text
auspex-core/src/authorization.rs
auspex-core/src/local_omegon_discovery.rs
auspex-core/src/local_omegon_probe.rs
auspex-core/src/operational_profile.rs
auspex-core/src/gateway_projection.rs
```

## Relationship to current MVP

This lane follows the local attach persistence/reprobe work. The immediate order should be:

1. Persist and rehydrate local attach state.
2. Add freshness/reprobe semantics.
3. Add Nex substrate inspect-only COP panel.
4. Add sandbox launch planning.
5. Launch a constrained Omegon sandbox as AuspexOwned.

## Open Questions
