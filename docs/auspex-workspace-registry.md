---
id: auspex-workspace-registry
title: "Workspace registry, working directories, and trust"
status: exploring
parent: auspex-omegon-runtime-control-plane
tags: [runtime-control]
open_questions:
  - "Can one workspace identity point to changing branches, or must checkout identity include repository and worktree path?"
  - "How are symlinks and roots outside the project handled without path-traversal or trust escalation?"
  - "Which workspace operations create checkouts or worktrees versus only registering existing paths?"
  - "[assumption] Workspace registration does not itself confer trust or write permission."
dependencies: []
related:
  - auspex-omegon-runtime-control-plane
  - auspex-worker-profiles
---

# Workspace registry, working directories, and trust

## Overview

Define stable workspace identities over canonical working directories, repository/check-out metadata, branch state, writability, trust grants, and launch placement.

## Decisions

_No decisions accepted yet._

## Research

### Control-surface matrix baseline

The initial Omegon surface inventory is recorded in the parent node, [[auspex-omegon-runtime-control-plane]]. This node owns one bounded part of that control-plane substrate and must preserve the parent authority, persistence, and transport distinctions.

## Open Questions

The structured questions in frontmatter are the current frontier. Before deciding this node, explicitly ask: **What assumptions is this design making that have not been stated?** Record each answer as an `[assumption]` question until validated.
