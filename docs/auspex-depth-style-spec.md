---
title: "Auspex depth-first visual style specification"
status: exploring
tags: [ui, design, theme, depth, skeuomorphism]
---

# Auspex depth-first visual style specification

Auspex should reject flat UI minimalism without regressing into theatrical skeuomorphism. The target aesthetic is a **precision operator console** with structured false depth, tactile hierarchy, and strong semantic clarity.

## Design thesis

> Reintroduce depth with discipline.

The UI should communicate hierarchy and affordance through raised surfaces, inset wells, edge highlights, and short directional shadows. Depth is structural information, not decoration.

## What this style is

- dark, calm, instrument-like
- layered surfaces with explicit depth roles
- tactile controls and container hierarchy
- crisp semantic states (success, warning, danger, active)
- restrained neumorphic influence

## What this style is not

- flat SaaS dashboard UI
- fake materials (leather, brushed metal, glass gloss)
- oversized bevels or toy-like controls
- embossed body text
- low-contrast decorative neumorphism

## Surface model

### Level 0 — shell
The global application background. Deep matte surface, visually receding.

### Level 1 — trays
Structural containers such as rails and workspace shells. Slightly raised from the shell with subtle edge contrast.

### Level 2 — cards
Primary panels and focal surfaces. More explicit highlight/shadow pairing and local separation.

### Inset wells
Pressed-in surfaces for subordinate or receiving content:
- filters
- search controls
- transcript detail bodies
- list wells
- code/log content regions

### Pressed state
Interactive surfaces under active interaction. Reduced outer lift, stronger inset emphasis.

## Depth-to-meaning mapping

- **raised** → actionable, foreground, selected, focal
- **inset** → receiving, scoped, subordinate, filterable
- **neutral** → noninteractive layout plane
- **pressed** → active interaction
- **sharp semantic accent** → meaning beats depth

## Semantic rule

Depth never replaces semantics. Warning/failure/success states must remain explicit through color and typography. Depth supports interpretation; it does not carry it alone.

## Token families

### Structural tokens
- shell background
- tray background
- raised panel background
- inset panel background
- soft border
- strong border

### Light/shadow tokens
- outer lift shadow
- top-left highlight edge
- inset shadow
- inset highlight
- selected accent ring

### Semantic tokens
- info
- success
- warning
- danger
- muted/unavailable

### Shape tokens
- radius-sm
- radius-md
- radius-lg
- spacing scale

## Component mapping

### Top bar
- low-height structural tray
- active tab is raised
- inactive tabs rest on a quieter inset plane
- status chips remain crisp, not puffy

### Left rail
- inset tray container
- compact raised summary cards
- no full investigative workflows here

### Audit workspace
Pilot surface for the redesign.

- outer workspace: neutral shell
- control column: inset tray with nested raised summary card
- result/detail area: raised panel
- entry list rows: modest raised cards with hover lift
- filters: inset controls
- selected detail: strongest local elevation

### Session / Scribe
- grouped raised panels
- canonical Instance panel before Dispatcher binding
- warnings remain sharper and flatter than decorative panels

### Transcript
- outer block may be softly raised
- disclosure bodies should be inset
- tool/system/error semantics remain clearer than depth language

## Interaction states

### Hover
Slight increase in lift or border clarity. No dramatic glow.

### Active / pressed
Reduced outer lift + pressed-in visual response.

### Selected
Accent ring + clearer elevation.

### Disabled
Muted contrast; preserve silhouette.

## Guardrails

- keep borders even when using shadows
- do not rely on shadow direction alone for meaning
- do not neumorphize alerts or badges
- do not over-round dense data panels
- do not apply heavy gradients to transcript content areas

## First implementation slice

1. top chrome and workspace tabs
2. Audit workspace controls, list, and detail surfaces
3. audit summary card and filter controls
4. selected/hover/pressed states

## Review criteria

The redesign succeeds if:
- hierarchy is easier to parse at a glance
- filter controls feel tactile without becoming decorative
- Audit feels like a first-class investigative workspace
- semantic warnings/errors remain sharper than ambient panels
- the UI is unmistakably non-flat without becoming kitsch
