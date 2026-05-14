---
id: auspex-desktop-shell-frame
title: "Auspex desktop shell frame and workspace chrome"
status: exploring
tags: []
open_questions:
  - "Should Work and Session remain directly navigable as first-class workspace tabs anywhere in desktop shell v2, or be fully redistributed into right-rail inspection and bottom/global instrumentation surfaces?"
  - "What interaction should top-left and bottom-left identity boxes use for deeper details — modal, popover, expandable panel, or routed detail pane?"
dependencies: []
related: []
---

# Auspex desktop shell frame and workspace chrome

## Overview

Define the desktop shell frame for Auspex: global top/bottom bars, inner left/right rails, center workspace host, corner-box semantics, and top-level workspace taxonomy.

This is not a styling pass. It is the first real desktop shell architecture for Auspex.

The current app shell is still a vertically stacked interface: header, controls, notices, status cards, activity strip, tab bar, and content pane. That is enough for bootstrap and migration work, but it is not the right long-term operator shell.

The desktop shell should instead expose a durable frame with stable spatial meaning.

## Decisions

### Desktop shell uses global top/bottom bars plus inner left/right rails around a normal-aspect center workspace

**Status:** accepted

**Rationale:** Auspex needs stable spatial separation for global chrome, scoped navigation, primary workspace content, contextual inspection, and instrumentation. A five-region frame preserves a cognitively primary center pane while making system state legible around it.

### Top-center workspace taxonomy is COP, Chat, Graph, Workflow, and Audit

**Status:** accepted

**Rationale:** These are first-class peer workspaces rather than arbitrary screens: COP is the live operator surface, Chat is the primary embedded Omegon interaction surface, Graph is deployed-agent topology, Workflow is the `omegon-flow` handoff surface, and Audit is the lifecycle record. Scribe is no longer active workspace chrome; task tracking belongs to Flynt's board model, Sentry owns execution, and Auspex observes and dispatches across that boundary.

### Left rail groups by project/workspace first, then shows constituent sessions/agents

**Status:** accepted

**Rationale:** Project/repo is the durable work boundary, while sessions and agents are runtime instances within that boundary. Flattening them into a single list would blur scope selection and runtime topology.

### Corner boxes have distinct semantics: top-left runtime identity, bottom-left org/operator identity, bottom-right intentionally reserved

**Status:** accepted

**Rationale:** Corners should carry persistent global identity and trust surfaces, not arbitrary workflow details. Top-left conveys shell/runtime placement, bottom-left conveys org/operator identity, and bottom-right remains a reserved aperture until a future global concern truly earns the space.

## Open Questions

- Should Work and Session remain directly navigable as first-class workspace tabs anywhere in desktop shell v2, or be fully redistributed into right-rail inspection and bottom/global instrumentation surfaces?
- What interaction should top-left and bottom-left identity boxes use for deeper details — modal, popover, expandable panel, or routed detail pane?
