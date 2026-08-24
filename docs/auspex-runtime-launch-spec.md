---
id: auspex-runtime-launch-spec
title: "Resolved runtime launch specifications"
status: exploring
parent: auspex-omegon-runtime-control-plane
tags: [runtime-control]
open_questions:
  - "Which fields may be overridden at launch, and can overrides only narrow authority?"
  - "How are secret references represented without embedding secret values in the launch artifact?"
  - "What exact provenance is retained for inherited profile and workspace revisions?"
  - "[assumption] Every launched primary, child, and background runtime can retain a launch-spec identifier and digest."
dependencies: []
related:
  - auspex-omegon-runtime-control-plane
  - auspex-worker-profiles
---

# Resolved runtime launch specifications

## Overview

Define deterministic resolution of profile, workspace, capability policy, permissions, model routing, environment references, limits, and operator-approved narrowing overrides into an immutable launch specification and digest.

## Decisions

_No decisions accepted yet._

## Research

### Control-surface matrix baseline

The initial Omegon surface inventory is recorded in the parent node, [[auspex-omegon-runtime-control-plane]]. This node owns one bounded part of that control-plane substrate and must preserve the parent authority, persistence, and transport distinctions.

## Open Questions

The structured questions in frontmatter are the current frontier. Before deciding this node, explicitly ask: **What assumptions is this design making that have not been stated?** Record each answer as an `[assumption]` question until validated.
