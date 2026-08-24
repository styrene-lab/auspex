---
id: theme-token-hierarchy
title: "Tweak.cn theming port + hierarchical token system"
status: exploring
tags: []
open_questions:
  - "[assumption] omegon/styrene-rs do not already ship a shared/canonical theme crate — verify before copying flynt's theme.rs, and prefer extracting a shared crate if one does not exist"
dependencies: []
related: []
---

# Tweak.cn theming port + hierarchical token system

## Overview

Port the styrene-ecosystem tweak.cn theming model (reference: flynt-app/src/theme.rs) to auspex, built on a strict token hierarchy that prevents CSS/JS slop:

Layer 0 — theme tokens: shadcn/tweak.cn vocabulary (--background, --foreground, --card, --primary, --muted, --muted-foreground, --accent, --destructive, --border, --input, --ring, --radius) defined at :root. This is the only layer a theme import replaces.

Layer 1 — semantic aliases: auspex's existing --bg-*, --surface-*, --text-*, --border-*, --accent-*, --agent-* tokens, re-derived from Layer 0 where possible. Specialized values (per-role surfaces, status borders) stay here but must reference Layer 0 or be explicitly theme-invariant.

Layer 2 — component rules: consume only Layer 1/0 vars. Raw `#hex` and `rgba()` literals in component rule blocks are migration targets, not theme inputs.

Machinery port (ThemeLibrary, inline_vars on root shell, presets JSON, operator settings persistence) follows after token consolidation proves out. Consider extracting flynt's theme.rs into a shared styrene-rs crate rather than copying.

## Decisions

### Three-layer token hierarchy: theme tokens → semantic aliases → component rules

**Status:** accepted

**Rationale:** A fixed hierarchy lets theme imports replace base tokens without bypassing Auspex semantic roles or coupling components to one palette.

## Open Questions

- [assumption] omegon/styrene-rs do not already ship a shared/canonical theme crate — verify before copying flynt's theme.rs, and prefer extracting a shared crate if one does not exist
