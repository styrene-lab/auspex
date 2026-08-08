---
id: auspex-runtime-menu-projection
title: "Runtime administration menu projection"
status: exploring
parent: auspex-omegon-runtime-control-plane
tags: [runtime-control]
open_questions:
  - "How should unavailable, degraded, local-only, and unsupported actions appear?"
  - "Which extension-contributed actions may enter menus, and what metadata must they declare?"
  - "How should the UI distinguish session-only application from persistent profile edits and restart-required changes?"
  - "[assumption] Menus can share one semantic projection across Dioxus and future operator surfaces."
dependencies: []
related:
  - auspex-omegon-runtime-control-plane
  - auspex-worker-profiles
---

# Runtime administration menu projection

## Overview

Define the renderer-neutral information architecture and action metadata used to build Auspex menus from the canonical catalog plus live capability, authority, and resource state.

## Decisions

_No decisions accepted yet._

## Research

### Control-surface matrix baseline

The initial Omegon surface inventory is recorded in the parent node, [[auspex-omegon-runtime-control-plane]]. This node owns one bounded part of that control-plane substrate and must preserve the parent authority, persistence, and transport distinctions.

## Open Questions

The structured questions in frontmatter are the current frontier. Before deciding this node, explicitly ask: **What assumptions is this design making that have not been stated?** Record each answer as an `[assumption]` question until validated.
