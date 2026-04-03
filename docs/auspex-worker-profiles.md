---
id: auspex-worker-profiles
title: "Auspex worker profiles and knob allocation"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions:
  - "Which knobs are profile defaults versus per-instance overrides?"
dependencies: []
related:
  - auspex-multi-agent-runtime
---

# Auspex worker profiles and knob allocation

## Overview

Define profile-based allocation of model/provider/thinking/tool/runtime knobs across supervisor, primary interactive, cheap subtask, and background service Omegon workers.

## Decision

### Worker instantiation is profile-driven

**Status:** accepted

Auspex should instantiate workers from named profiles rather than hand-setting all knobs per instance. Per-instance overrides are still allowed, but profiles remain the source of truth.

## First-pass profiles

### `primary-interactive`
Used for the main desktop-facing agent.

```json
{
  "model_strategy": {
    "preferred": ["anthropic:claude-sonnet-4-6", "openai:gpt-4.1"],
    "fallback": ["anthropic:claude-haiku", "openai:gpt-4.1-mini"]
  },
  "thinking_level": "medium",
  "context_class": "clan",
  "tool_policy": "full",
  "memory_mode": "full"
}
```

### `supervisor-heavy`
Used for orchestration/scheduling/complex routing decisions.

```json
{
  "model_strategy": {
    "preferred": ["anthropic:claude-sonnet-4-6", "openai:gpt-4.1"],
    "fallback": ["anthropic:claude-haiku"]
  },
  "thinking_level": "high",
  "context_class": "legion",
  "tool_policy": "full",
  "memory_mode": "full"
}
```

### `cheap-subtask`
Used for delegated child work by default.

```json
{
  "model_strategy": {
    "preferred": ["anthropic:claude-haiku", "gpt-spark", "openai:gpt-4.1-mini"],
    "fallback": ["local:qwen2.5-coder"]
  },
  "thinking_level": "low",
  "context_class": "squad",
  "tool_policy": "restricted",
  "memory_mode": "minimal",
  "max_runtime_seconds": 900
}
```

### `background-service`
Used for long-running detached service workers.

```json
{
  "model_strategy": {
    "preferred": ["anthropic:claude-haiku", "openai:gpt-4.1-mini"],
    "fallback": ["local:qwen2.5-coder"]
  },
  "thinking_level": "minimal",
  "context_class": "maniple",
  "tool_policy": "bounded",
  "memory_mode": "project-only"
}
```

## First-pass knobs

Profiles may allocate defaults for:

- provider preference
- model preference/fallbacks
- thinking level
- context class
- tool policy
- memory mode
- runtime limit
- cost limit
- parallelism limit
- network policy

## Override model

Each worker instance may override selected profile defaults.

Allowed overrides should include:

- model/provider
- thinking level
- runtime limit
- cost limit
- namespace/resources for OCI/Kubernetes placement

## Constraint

Supervisor-class workers should prefer stronger models.
Delegated child workers should default to cheaper/free models unless the operator or orchestration policy explicitly escalates them.
