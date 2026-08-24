+++
title = "Arwes Dashboard Assessment — Lady of Shalott"
tags = ["design","omegon-web","arwes","ui"]
+++

# Arwes Dashboard Assessment — Lady of Shalott

---
title: Arwes Dashboard Assessment — Lady of Shalott
status: exploring
tags: [design, omegon-web, arwes, ui]
source: https://www.myxouz.com/2021/12/lady-of-shalott-first-version-of-our-home-dashboard/
---

# Arwes Dashboard Assessment — Lady of Shalott

Design DNA extracted from the Lady of Shalott home dashboard (Arwes, 2560×1440 kiosk),
assessed as reference for the [[omegon-web-mock|Omegon Web]] surface.

## Core principle: modules on a field, not cards in a grid

- **No outer frame**; content bleeds edge to edge (wall/mirror kiosk).
- ~12 modules on one dark field in a strict 3-column grid.
- Modules separated by **negative space + a header rule**, not borders.
- Charts sit **directly on the background** — zero card chrome.
- Clipped Arwes frames are **rare and semantic** — used only for interactive/grouped
  entities (Resident cards, vacuum percentage tabs), never as layout wallpaper.

This is the inverse of the mock's original "every panel is a lit frame" approach.

## Color: a deliberate two-channel semantic split

| Channel | Color | Used for |
|---|---|---|
| **Cyan** `~#1ec8da` | structure | headers, labels, icons, chart fills, rules, title |
| **Amber** `~#e0975a` | live data | event dates, arrival times, "Few seconds ago", %, headline text, map path |

Cyan = the instrument; amber = the reading. Amber is used heavily but **only ever on
data values**, so density reads as disciplined, not technicolor. Lesson: amber must be
**semantic, not decorative**.

## Typography: hard sans/mono split by data role

- Humanist sans (Titillium Web — Arwes default) for titles, headers, prose.
- **Monospace for all telemetry/tabular data**: logs, time columns, percentages, timestamps.
- Title carries a blinking-cursor underscore (terminal motif).
- Section headers are **sentence case**; ALL-CAPS reserved for tiny meta-labels.

## Header pattern (adopted)

`▌ Indoor Environment ─────────────────` — bright cyan tick bar, label, then a thin rule
extending to fill module width. Lightweight, infinitely repeatable, scales to any width.
Far lighter than a boxed panel heading.

## Data-viz language

- Monochrome cyan **filled area charts**, vertical gradient to transparent, no axes/legend.
- Floor-plan map as single-color amber line-art with live robot path.
- Logs are raw dimmed monospace tables — data itself is the texture.

## What NOT to copy

- Zero outer frame suits a kiosk, not a windowed desktop app (keep the HUD frame).
- Author notes animation cost tanked performance — be selective with Arwes frame draws.
- Decorative rotated-character spine is pure ornament.
- Read-only non-scrolling wall; our surface is interactive chat.

## Gap analysis → Omegon Web mock

| Element | Reference | Mock (pre) | Action |
|---|---|---|---|
| Chrome density | headers + rules; frames rare | every panel boxed + bracketed | demote panels to header+rule; reserve frames |
| Amber | semantic: values only | removed entirely | reintroduce, bound to values |
| Headers | tick + extending rule, sentence case | boxed, all-caps | adopt tick+rule header |
| Mono | all telemetry/tabular | labels only | push timestamps/counts/logs to mono |
| Charts | unboxed area charts | none | add sparklines (context/token/rate) |
| Frames | punctuation (cards, readouts) | wallpaper | restrict to interactive groups |

## Synthesis

Density works because **structure is cyan and quiet, data is amber and loud, chrome is
nearly absent**. Highest-leverage change for the mock: semantic cyan/amber split +
lightweight tick-rule headers — raises data density without raising visual noise.

## Implementation status

- [x] Tick-rule header pattern (`.owm-eyebrow` → tick + extending rule)
- [x] Semantic amber on live values (`.owm-rail-card dd` → `--signal`)
- [x] Demote rail/transcript panels from full frames to header+rule
- [x] Add sparklines for context/token/rate
- [ ] Push remaining telemetry (timestamps, counts) to mono + amber
