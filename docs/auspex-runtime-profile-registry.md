---
id: auspex-runtime-profile-registry
title: "Runtime profile registry and editing"
status: exploring
parent: auspex-omegon-runtime-control-plane
tags: [runtime-control]
open_questions:
  - "What is the optimistic-concurrency and revision format for profile writes?"
  - "Which profile fields can be applied live and which require reload, restart, or next launch?"
  - "How are project profile mutations reviewed when they modify repository files?"
  - "[assumption] Omegon profile persistence can expose atomic writes and validation through a typed management boundary."
dependencies: []
related:
  - auspex-omegon-runtime-control-plane
  - auspex-worker-profiles
---

# Runtime profile registry and editing

## Overview

Define typed profile list, inspect, create, copy, patch, validate, delete, select, import, export, and revision operations while preserving Omegon-native profile semantics and user/project precedence.

## Decisions

_No decisions accepted yet._

## Research

### Control-surface matrix baseline

The initial Omegon surface inventory is recorded in the parent node, [[auspex-omegon-runtime-control-plane]]. This node owns one bounded part of that control-plane substrate and must preserve the parent authority, persistence, and transport distinctions.

## Open Questions

The structured questions in frontmatter are the current frontier. Before deciding this node, explicitly ask: **What assumptions is this design making that have not been stated?** Record each answer as an `[assumption]` question until validated.
