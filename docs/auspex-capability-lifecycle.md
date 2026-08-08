---
id: auspex-capability-lifecycle
title: "Skill, extension, plugin, and package lifecycle"
status: exploring
parent: auspex-omegon-runtime-control-plane
tags: [runtime-control]
open_questions:
  - "What provenance and integrity evidence is required for URL, path, Git, and Armory installs?"
  - "What is the common capability identity across installed artifacts, profile policy, and loaded runtime generations?"
  - "Which updates support live reload and which require runtime restart?"
  - "[assumption] Skill activation and extension loading can be represented with a shared lifecycle vocabulary without erasing their differences."
dependencies: []
related:
  - auspex-omegon-runtime-control-plane
  - auspex-worker-profiles
---

# Skill, extension, plugin, and package lifecycle

## Overview

Define joined inventory and lifecycle operations for skills, extensions, plugins, packages, Armory sources, and agent catalogs. Keep installed, permitted, activated, loaded, and update states distinct.

## Decisions

_No decisions accepted yet._

## Research

### Control-surface matrix baseline

The initial Omegon surface inventory is recorded in the parent node, [[auspex-omegon-runtime-control-plane]]. This node owns one bounded part of that control-plane substrate and must preserve the parent authority, persistence, and transport distinctions.

## Open Questions

The structured questions in frontmatter are the current frontier. Before deciding this node, explicitly ask: **What assumptions is this design making that have not been stated?** Record each answer as an `[assumption]` question until validated.
