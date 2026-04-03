---
id: auspex-worker-inheritance
title: "Auspex worker inheritance and policy propagation"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions: []
dependencies: []
related:
  - auspex-multi-agent-runtime
  - auspex-worker-profiles
---

# Auspex worker inheritance and policy propagation

## Overview

Define how supervised-child and detached-service Omegon workers inherit or diverge from the primary driver's auth, workspace, memory, tool policy, and profile settings.

## Decisions

### Child workers inherit workspace identity by default

**Status:** accepted

A supervised-child worker should inherit:

- workspace identity
- cwd or worktree binding
- repository/branch context
- parent instance id

This preserves task locality and makes child outputs attributable to the same project context.

### Child workers do not inherit full mutable session state by default

**Status:** accepted

A supervised-child worker should **not** receive the parent worker's full mutable conversation/session state by default.

Instead, Auspex should pass an explicit task-scoped projection, for example:

- task prompt / decomposition slice
- relevant spec binding
- selected work context
- optional focused-node summary

This avoids accidental context explosion and reduces hidden coupling between workers.

### Memory inheritance defaults to reference-based or project-scoped, not full clone

**Status:** accepted

The default should be one of:

- `project-only`
- `reference-based`
- `minimal`

but **not** a blind full memory clone.

The parent may pass:
- relevant design node ids
- relevant OpenSpec change ids
- relevant memory fact ids

rather than copying the full active mental state.

### Auth inheritance should propagate capability references, not duplicate secrets where possible

**Status:** accepted

Child workers should inherit the ability to use the same provider auth/material, but through references or mounted secret backends rather than raw copied credentials where possible.

Implications by backend:

- `local-process`: inherit environment / keychain access through the same host user context
- `local-detached`: inherit secret references persisted by Auspex
- `oci-container`: mount or inject scoped secrets at runtime
- `kubernetes`: use Kubernetes Secrets or external secret references

### Child policy inheritance is narrowing by default

**Status:** accepted

A child worker may inherit broad defaults from the primary driver, but the effective child policy should normally be **more restricted**, not broader.

Typical narrowing examples:
- lower model tier
- lower thinking level
- tighter runtime/cost bounds
- reduced tool/network access
- reduced memory scope

### Detached-service workers inherit less than supervised children

**Status:** accepted

Detached-service workers should inherit:
- workspace binding
- selected auth capability references
- selected profile defaults

But they should not implicitly inherit a live transient session context from the primary driver. Detached workers are intentionally more independent and must be re-attachable later.

## First-pass inheritance matrix

| Concern | primary → supervised-child | primary → detached-service |
|---|---|---|
| Workspace cwd/worktree | inherit | inherit |
| Branch / repo identity | inherit | inherit |
| Parent instance id | inherit | inherit |
| Full conversation transcript | no | no |
| Task-scoped prompt/context | yes | yes |
| Focused design/OpenSpec refs | yes | optional |
| Full memory state clone | no | no |
| Project memory refs | yes | yes |
| Provider auth capability | inherit by reference | inherit by reference |
| Raw credentials copy | avoid | avoid |
| Tool policy | narrow | narrow or fixed |
| Model tier | usually cheaper | profile-owned |
| Thinking level | usually lower | profile-owned |

## First-pass propagation object

Auspex should resolve an explicit propagation object before instantiating a worker.

```json
{
  "workspace": {
    "cwd": "/repo/path",
    "workspace_id": "repo:8f2f4c1",
    "branch": "main"
  },
  "task_context": {
    "task_id": "clv-child-2",
    "prompt": "Implement the schema projection for tool cards",
    "design_refs": ["auspex-data-model-v2"],
    "spec_refs": ["auspex-data-model-v2"],
    "memory_refs": ["fact_123", "fact_456"]
  },
  "auth": {
    "provider_refs": ["anthropic", "openai"],
    "secret_mode": "reference"
  },
  "policy": {
    "base_profile": "cheap-subtask",
    "resolved_model": "anthropic:claude-haiku",
    "thinking_level": "low",
    "tool_policy": "restricted",
    "memory_mode": "project-only"
  }
}
```

## Constraint

Inheritance must be explicit and inspectable. If Auspex cannot explain why a child worker had a given tool, model, or secret capability, the inheritance model is too implicit.
