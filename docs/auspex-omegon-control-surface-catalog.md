---
id: auspex-omegon-control-surface-catalog
title: "Omegon canonical control-surface catalog"
status: exploring
parent: auspex-omegon-runtime-control-plane
tags: [runtime-control]
open_questions:
  - "Which registry is authoritative when command metadata, backend metadata, and runtime behavior disagree?"
  - "How will capability negotiation report operations unavailable on an attached Omegon version?"
  - "[assumption] The catalog can be versioned independently while retaining stable canonical action IDs."
dependencies: []
related:
  - auspex-omegon-runtime-control-plane
  - auspex-worker-profiles
---

# Omegon canonical control-surface catalog

## Overview

Inventory and normalize every Omegon operator control across CLI, slash, TUI menus, ACP, IPC, WebSocket, HTTP, model tools, and extension RPC. Define canonical domains, resource kinds, mutability, required authority, persistence, runtime effect, side effects, and preferred/fallback transports.

## Decisions

_No decisions accepted yet._

## Research

### Control-surface matrix baseline

The initial Omegon surface inventory is recorded in the parent node, [[auspex-omegon-runtime-control-plane]]. This node owns one bounded part of that control-plane substrate and must preserve the parent authority, persistence, and transport distinctions.

## Open Questions

The structured questions in frontmatter are the current frontier. Before deciding this node, explicitly ask: **What assumptions is this design making that have not been stated?** Record each answer as an `[assumption]` question until validated.
