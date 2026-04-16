---
id: auspex-worker-profiles
title: "Auspex worker profiles and knob allocation"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions: []
dependencies: []
related:
  - auspex-multi-agent-runtime
  - auspex-worker-inheritance
---

# Auspex worker profiles and knob allocation

## Overview

Define profile-based allocation of model/provider/thinking/tool/runtime knobs across supervisor, primary interactive, cheap subtask, and background service Omegon workers.

## Decisions

### Profiles are the primary source of worker policy

**Status:** accepted

Instances may override profile defaults, but profiles are the source of truth for normal allocation.

### Supervisor and primary-driver profiles prefer stronger models

**Status:** accepted

The orchestration/supervision path should use more capable models by default.

### Child and background profiles default to cheaper models

**Status:** accepted

Delegated work should default to Haiku, GPT-spark, or equivalent low-cost/local options unless explicitly escalated.

### Dispatcher selection is model-selectable but constrained by role-eligible profiles

**Status:** proposed

The operator-facing session dispatcher should be selectable like any other model-facing worker, but not every profile/model combination should be valid for the dispatcher role. Dispatcher selection should be filtered by profiles intended for orchestration-capable primary workers rather than allowing cheap child/background defaults to become the main dispatcher implicitly.

### Inheritance resolves before profile overrides are finalized

**Status:** accepted

Effective worker policy should be resolved in this order:

1. role/profile defaults
2. inherited narrowing constraints
3. explicit per-instance overrides

This prevents parent inheritance from silently broadening a restricted profile.

## Canonical config format

Pkl is the canonical config format, with TOML as fallback. Schema lives in `pkl/WorkerProfile.pkl`. Config is loaded from `~/.config/auspex/`:

```text
~/.config/auspex/worker-profiles.pkl    # preferred
~/.config/auspex/worker-profiles.toml   # fallback
```

Evaluation: `rpkl::from_config()` shells out to the `pkl` CLI, validates against the schema, and deserializes directly into Rust structs via serde. Same pattern as omegon's `agent_manifest.rs`.

### Pkl example

```pkl
amends "WorkerProfile.pkl"

version = 1

profiles {
  ["primary-interactive"] {
    role = "primary-driver"
    preferred_models { "anthropic:claude-sonnet-4-6"; "openai:gpt-4.1" }
    fallback_models { "anthropic:claude-haiku"; "openai:gpt-4.1-mini" }
    thinking_level = "medium"
    context_class = "clan"
    tool_policy = "full"
    memory_mode = "full"
  }
  ["supervisor-heavy"] {
    role = "primary-driver"
    preferred_models { "anthropic:claude-sonnet-4-6"; "openai:gpt-4.1" }
    fallback_models { "anthropic:claude-haiku" }
    thinking_level = "high"
    context_class = "legion"
    tool_policy = "full"
    memory_mode = "full"
  }
  ["cheap-subtask"] {
    role = "supervised-child"
    preferred_models { "anthropic:claude-haiku"; "gpt-spark"; "openai:gpt-4.1-mini" }
    fallback_models { "local:qwen2.5-coder" }
    thinking_level = "low"
    context_class = "squad"
    tool_policy = "restricted"
    memory_mode = "minimal"
    max_runtime_seconds = 900
    max_cost_usd = 0.50
    parallelism_limit = 4
    network_policy = "restricted"
  }
  ["background-service"] {
    role = "detached-service"
    preferred_models { "anthropic:claude-haiku"; "openai:gpt-4.1-mini" }
    fallback_models { "local:qwen2.5-coder" }
    thinking_level = "minimal"
    context_class = "maniple"
    tool_policy = "bounded"
    memory_mode = "project-only"
    max_cost_usd = 5.00
  }
}
```

## Knobs owned by profiles

Profiles may define defaults for:

- `preferred_models`
- `fallback_models`
- `thinking_level`
- `context_class`
- `tool_policy`
- `memory_mode`
- `max_runtime_seconds`
- `max_cost_usd`
- `parallelism_limit`
- `network_policy`

## Allowed per-instance overrides

A worker instantiation request may override:

- explicit model/provider
- thinking level
- runtime limit
- cost limit
- backend resources/namespace/image
- tool policy restrictions tighter than profile defaults

## Constraint

Per-instance overrides should be additive or narrowing. They should not silently expand a restricted profile into a broad-privilege worker without an explicit operator decision.
