---
id: auspex-runtime-reconciliation
title: "Runtime observation, drift, and reconciliation"
status: exploring
parent: auspex-omegon-runtime-control-plane
tags: [runtime-control]
open_questions:
  - "Which differences are benign runtime state versus actionable configuration drift?"
  - "What is the lifecycle contract for apply-for-session, save-to-profile, reload, restart, detach, stop, and shutdown?"
  - "How are partially applied changes and failed restarts represented and recovered?"
  - "[assumption] Omegon can expose enough effective-state metadata to compare against an Auspex launch specification."
dependencies: []
related:
  - auspex-omegon-runtime-control-plane
  - auspex-worker-profiles
---

# Runtime observation, drift, and reconciliation

## Overview

Define runtime-instance observation and intended-versus-observed comparison for working directory, effective model, loaded capabilities, permissions, configuration generation, process identity, and lifecycle.

## Decisions

_No decisions accepted yet._

## Research

### Control-surface matrix baseline

The initial Omegon surface inventory is recorded in the parent node, [[auspex-omegon-runtime-control-plane]]. This node owns one bounded part of that control-plane substrate and must preserve the parent authority, persistence, and transport distinctions.

## Open Questions

The structured questions in frontmatter are the current frontier. Before deciding this node, explicitly ask: **What assumptions is this design making that have not been stated?** Record each answer as an `[assumption]` question until validated.
