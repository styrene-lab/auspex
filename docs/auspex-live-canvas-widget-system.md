---
id: auspex-live-canvas-widget-system
title: "Auspex live canvas widget system"
status: exploring
tags: [ui, canvas, widgets, layout, hud, responsive]
related:
  - auspex-desktop-shell-frame
  - auspex-depth-style-spec
  - controller-architecture
  - vision
---

# Auspex live canvas widget system

## Overview

Define the next shell evolution for Auspex: a stable global canvas hosting modular operator widgets with explicit snap-layout mechanics, while preserving the existing shell frame as the migration substrate.

This is **not** a freeform floating-window design and **not** a web-dashboard port. The target is a live operator HUD built from disciplined application panels.

## Why this exists

Auspex currently has the right raw ingredients for an operator shell:
- durable top chrome
- left/right rails
- central work area
- structured control and metadata panels
- controller-owned state and telemetry projection
- tokenized surface/depth system in `assets/main.css`

But the shell is still compositionally rigid. As telemetry, routing, lifecycle, audit, and session surfaces grow, a fixed rail-first layout will either become cramped or force too much meaning into a few hard-coded columns.

The live canvas model solves that by changing the unit of composition from **screen section** to **widget module**, while keeping a stable shell and a truthful default layout.

## Design thesis

> Auspex should behave like a live instrument wall: stable global frame, modular widgets, state-driven emphasis, operator-customizable layout, and explicit snap discipline.

This means:
- the shell background and identity frame remain stable
- information appears as bounded widgets on a global canvas
- widget movement is allowed only through constrained drag/drop + snap rules
- customization layers on top of a canonical default, not instead of it
- mobile/web use the same widget model with different compositions, not a second truth model

## Non-goals

This system must **not** become:
- arbitrary pixel-position floating windows
- a cinematic sci-fi mockup full of decorative gauges
- a second state model separate from `AppController`
- a Tailwind/web-stack port disguised as Rust UI architecture
- a layout mode that makes the default shell unreadable without manual customization

## Relationship to the current shell

The existing shell-frame decisions remain valid:
- top/bottom bars still carry global chrome and durable identity
- the center remains the cognitively primary workspace region
- rails retain stable meaning during migration
- Chat / Scribe / Graph remain the major workspace taxonomy unless superseded elsewhere

The canvas system therefore **extends** the current shell instead of replacing it.

### Migration rule

Treat the current rails and panels as **seeded widget zones**.

That means:
- left rail content becomes left-zone widgets
- right inspection surfaces become right-zone widgets
- bottom/global instrumentation becomes bottom-zone widgets
- center workspace content becomes center-zone widgets or routed workspace hosts

The rails are not deleted first. They are gradually reinterpreted as widget containers.

## Core model

### Shell vs canvas

- **Shell** — stable structural frame: top chrome, identity anchors, global workspace controls, mode indicators, reserved global apertures
- **Canvas** — snap-driven composition layer where widgets are placed, resized, shown, hidden, and reordered
- **Widget** — bounded module with one operator responsibility and one primary information contract

### Widget properties

Every widget should eventually have a model equivalent to:

- `WidgetId` — stable layout identity
- `WidgetKind` — semantic widget type
- `WidgetZone` — allowed placement region(s)
- `GridPosition` — snapped row/column origin inside a zone
- `GridSpan` — width/height in snapped units
- `WidgetState` — visible, collapsed, pinned, focused, degraded
- `ResponsivePolicy` — full/compact/summary behavior per width class
- `CapabilityRequirements` — when the widget is meaningful or should hide/degrade

### Default zones

Initial zones should be conservative and map to the current shell:

- `top-status` — compact global metrics, session identity, runtime/route readouts
- `left-summary` — project/workspace/session summary widgets
- `center-primary` — transcript, audit, graph, or active task focus
- `right-inspector` — session/control-plane/provider/dispatcher inspection widgets
- `bottom-instrumentation` — compact lifecycle/telemetry/trend widgets
- `overlay-transient` — temporary overlays, edit affordances, snap previews, command palettes

These are **zones**, not absolute screens.

## Coordinate systems and background field

The live canvas must not use raw screen pixels as its canonical layout coordinate system.

### Canonical rule

Persist placement semantically:
- `zone`
- snapped grid origin
- snapped grid span

Do **not** persist canonical widget placement as absolute pixel `x/y` coordinates.

### Three coordinate layers

#### 1. Shell-space coordinates

Shell space defines the usable application frame after accounting for:
- top chrome
- bottom/global bars
- reserved shell apertures
- platform safe areas
- current viewport/window bounds

This produces the effective shell rect within which zones are laid out.

#### 2. Zone-grid coordinates

Each zone owns its own snapped logical grid.

Widget layout must be stored relative to the zone grid, not the raw viewport. This is what allows the same layout to survive:
- desktop resize
- web split-pane widths
- tablet/narrow layouts
- square aspect ratios
- future mobile projections

Representative placement model:

```rust
struct WidgetPlacement {
    zone: WidgetZone,
    col: u16,
    row: u16,
    col_span: u16,
    row_span: u16,
}
```

#### 3. Widget-local coordinates

Geometry inside a widget uses widget-local normalized coordinates.

Examples:
- SVG `viewBox`
- normalized sparkline coordinates
- compact chart domains
- local layout padding boxes

This keeps instruments portable and independent from shell-level aspect changes.

### Background field rule

The global canvas background is a **structural field**, not a fixed hero illustration.

Allowed background responsibilities:
- subtle grid/seam structure
- zone framing and alignment lines
- ambient field markers
- restrained instrument-wall linework derived from shell/zone geometry

Disallowed background responsibilities:
- one giant central reticle whose meaning depends on a fixed aspect ratio
- decorative full-screen HUD art tied to a specific monitor shape
- background geometry that carries primary operational meaning

If a large radial or complex geometric figure conveys meaning, it should be a **widget**, not the canvas background.

### Coordinate derivation rule

Background geometry must derive from:
- shell rect
- zone rects
- breakpoint/aspect policy

It must **not** derive from hard-coded desktop artboard coordinates.

### Aspect-ratio policy

The canvas model must support at least these layout classes:
- `wide`
- `medium`
- `square`
- `narrow`
- `portrait`

These classes should not merely scale the same placement. They may:
- change visible zones
- remap zone column counts
- collapse widgets into compact/summary mode
- move side zones into trays/drawers/sheets
- simplify or suppress background field detail

### Reflow over scale

When aspect ratio or viewport class changes, the system should prefer **recomposition** over naive geometric scaling.

Meaning:
- widgets keep semantic placement within a zone
- zones may reorder or collapse
- widget internals may switch to compact mode
- the background field may simplify

The system should avoid shrinking a desktop wall uniformly until it becomes unreadable.

### Persistence rule

Saved layouts should persist:
- widget identity
- zone assignment
- snapped placement
- span
- collapsed/pinned state where applicable

Saved layouts should not persist viewport-specific absolute pixel positions as the canonical source of truth.

## Layout behavior

### Canonical default layout

Auspex must ship with a canonical default layout that encodes operator priority.

That default layout answers:
- what is ambient vs focal
- what must always be visible
- what belongs near the center vs at the edge
- what degrades first on narrow widths

Custom layouts are layered on top of that default. They never replace the need for a well-designed default.

### Drag/drop rules

Drag/drop should only be available in an explicit **layout edit mode**.

Normal operation mode must prioritize:
- widget interaction
- transcript selection
- filter editing
- control activation
- route/dispatcher actions

Layout editing therefore needs a separate affordance and different interaction contract.

### Snap rules

Drag/drop must snap to a constrained grid.

Required properties:
- no free pixel placement
- bounded region placement only
- per-widget min/max spans
- visible snap preview
- collision resolution rules
- reset-to-default layout
- per-profile saved layout later, but not required in the first slice

### Resize rules

Resize should be limited and semantic:
- some widgets are fixed-height
- some support compact vs expanded only
- some support full-width center placement only

Avoid arbitrary resize freedom until the widget inventory proves it is needed.

## Widget inventory from current Auspex surfaces

The current shell already implies the first widget set.

### High-confidence early widgets

- `SessionSummaryWidget`
- `ProviderStatusWidget`
- `LifecycleRollupWidget`
- `ControlPlaneWidget`
- `DispatcherBindingWidget`
- `ActiveDelegatesWidget`
- `AuditSummaryWidget`
- `AuditFiltersWidget`
- `AuditDetailWidget`
- `TranscriptFocusWidget`
- `WorkContextWidget`
- `RouteSelectionWidget`

### Migration guidance

These widgets should be carved out from existing panels and screen sections before any full canvas-edit feature lands.

## Primary instrumentation areas

The live canvas should not begin as a generic wall of arbitrary widgets. Its first responsibility is to make the operator's current Omegon deployment legible.

### Area 1 — attached Omegon status (primary instrumentation plane)

This is the first-class instrumentation area and should dominate the default layout.

Questions this area must answer immediately:
- is the primary attached Omegon reachable?
- what role is it serving right now?
- what route/dispatcher is the operator currently speaking to?
- is the control plane healthy, degraded, stale, or detached?
- what model/profile/thinking tier is effectively active?

This area should be treated as the main operator truth surface before broader multi-instance analytics are shown.

### Area 2 — multi-Omegon deployment inventory

Once the primary attached Omegon state is legible, the next layer is deployment visibility across one-to-many Omegon instances.

Questions this area must answer:
- how many Omegon instances are currently known?
- how many are live vs stale/lost/detached?
- which ones have been seen recently?
- which roles exist in the pool: primary-driver, supervised-child, detached-service?
- which instances belong to the active session versus the broader registry?
- which control-plane endpoints are verified and usable?

This is effectively the operator-facing rendition of the multi-instance registry.

### Area 3 — interaction classes for runtime orchestration

Once deployment visibility exists, the canvas should distinguish two interaction classes:

#### Persistent serve-mode Omegon instances

These are long-running and durable workers/services.

Examples:
- background agents
- detached services
- persistent session dispatchers
- dedicated long-lived support agents under supervision

The canvas should surface:
- durable identity
- ownership/supervision status
- placement/backend
- last seen / lifecycle freshness
- attach/detach/reconnect authority
- whether the instance is currently routable for operator interaction

#### Temporary dispatches for fixed-known-single-task operations

These are narrower-lived worker instantiations for bounded tasks.

Examples:
- delegated subtasks
- one-shot fixed-purpose background operations
- short-lived supervised-child workers

The canvas should surface:
- parent dispatcher/session
- task purpose / binding
- current lifecycle state
- expected expiry or completion semantics
- result handoff back to dispatcher/operator transcript

These two classes should not be visually or semantically conflated. A durable serve-mode worker is not just a longer task card.

## Deployment-first default layout rule

The default live canvas should prioritize deployment and authority visibility before generalized telemetry ornamentation.

That means the first widget cluster should bias toward:
- attached Omegon truth
- route/dispatcher truth
- multi-instance inventory
- lifecycle freshness
- serve-mode vs temporary-dispatch classification

Only after that foundation is legible should additional trend or decorative instrumentation compete for space.

## Widget priorities implied by deployment-first instrumentation

The first canonical widget cluster should evolve around:
- `AttachedOmegonStatusWidget`
- `RouteSelectionWidget`
- `DispatcherBindingWidget`
- `DeploymentInventoryWidget`
- `LifecycleRollupWidget`
- `ServeModeWorkersWidget`
- `TemporaryDispatchesWidget`

These widgets map directly onto accepted runtime doctrine in [[auspex-multi-agent-runtime]], [[auspex-session-dispatcher]], and [[auspex-runtime-backends]].

## Visual system implications

The live canvas model depends on the existing depth-first visual spec, but shifts emphasis from rail composition to bounded widget composition.

### What should remain true

- depth remains structural, not decorative
- warnings/errors stay sharper and flatter than ambient panels
- text remains primary over graphics
- borders remain explicit even when shadows are present
- the shell background stays stable and visually receding

### What changes

- more surfaces become modular instrument cards
- headers become more like readout rails
- micro-metadata and status strips gain importance
- widgets need explicit focus, ambient, degraded, and pinned visual states
- the canvas background becomes a stable field behind widgets, not a decorative illustration

## Style-only vs SVG rule

The live canvas model increases the value of SVG, but does not justify using it everywhere.

### Style-only by default

Use CSS/style-only for:
- shell background and zones
- widget frames and headers
- key/value telemetry blocks
- metadata cards
- transcript blocks
- filters and controls
- route and dispatcher action panels

### SVG only when geometry adds comprehension

Use SVG for widgets whose meaning depends on exact geometry:
- lifecycle segmented ring
- tiny provider trend chart
- compact route topology mini-map
- ring/arc summaries where textual counts remain primary

Reject SVG when it becomes decorative sci-fi garnish.

## State and data ownership

The live canvas layout is a **presentation system**, not a new state owner.

### Ownership rule

`AppController` remains the owner of session/runtime/telemetry/route truth.

Widgets consume controller-projected data. They do not invent parallel caches or alternative state derivations.

### Consequence

A widget should be hideable, movable, or collapsible without changing the underlying application state model.

This preserves the rule from [[screen-bindings]]: one underlying state/cache, multiple projections.

## Responsive model: desktop, narrow web, mobile

The widget model must survive across desktop and web/mobile targets without cloning the desktop shell literally.

### Desktop / wide web

- multiple zones visible simultaneously
- inspector widgets persistent
- compact instrumentation visible in ambient form
- center region remains primary

### Narrow desktop / tablet / split-screen web

- center remains primary
- side zones collapse into trays/drawers/secondary stacks
- widgets switch to compact mode
- fewer simultaneous widgets remain visible

### Mobile

- single-focus composition
- summary-first widgets
- drill-down into detail widgets or routed views
- same widget semantics, different arrangement
- no attempt to preserve the full desktop wall simultaneously

### Responsive rule

Widgets need explicit full/compact/summary behavior, not simple shrink-to-fit.

## Motion rules

The live canvas can support more motion than the current shell, but motion must remain semantic.

Good motion:
- snap previews in layout mode
- subtle state transitions between ambient/active/degraded
- small trend or pulse changes on live telemetry
- focused widget lift or accent clarification

Bad motion:
- continuous decorative scanning effects
- always-on glow animation
- layout drift while idle
- cinematic effects that compete with readability

## Implementation phases

### Phase 1 — widgetization without drag/drop

Goal: keep the current shell, but convert major screen sections into explicit widget modules.

Expected file pressure:
- `src/app.rs`
- `src/screens.rs`
- `assets/main.css`

Outputs:
- widget component boundaries
- widget headers/frames/states
- zone-aware composition inside existing rails and center panes

### Phase 2 — canonical canvas layout model

Goal: add internal widget layout metadata and stable zone/grid placement without exposing operator editing yet.

Outputs:
- widget registry
- default layout schema
- zone grid model
- compact/full responsive behavior

### Phase 3 — layout edit mode and snap mechanics

Goal: allow drag/drop + snap for supported widgets.

Outputs:
- edit mode affordance
- drag handles
- snap preview overlay
- move/reflow rules
- reset-to-default

### Phase 4 — saved layouts and task-oriented presets

Goal: persist operator-approved layouts and optionally define task presets.

Possible future presets:
- chat-heavy
- audit-heavy
- telemetry-heavy
- routing/dispatcher-heavy

## Immediate implementation guidance

### Files to touch first

- `assets/main.css`
  - define widget-frame semantics, zone surfaces, and ambient/focused/degraded states
- `src/screens.rs`
  - carve Session/Audit/telemetry sections into widget-ready components
- `src/app.rs`
  - treat shell regions as zone hosts rather than fixed screen slabs

### First widgets to formalize

1. `AuditSummaryWidget`
2. `AuditFiltersWidget`
3. `SessionSummaryWidget`
4. `ProviderStatusWidget`
5. `LifecycleRollupWidget`
6. `ControlPlaneWidget`

These are all already present as bounded control/metadata surfaces and can be migrated with style-first techniques before any drag/drop implementation.

## Review criteria

The live canvas direction succeeds if:
- the default shell becomes more modular without losing operator clarity
- existing rails feel like intentional widget zones rather than hard-coded sidebars
- state emphasis becomes easier to read at a glance
- desktop customization can be added later without rewriting the controller model
- mobile/web can project the same widget semantics without cloning the desktop wall
- SVG remains selective and high-signal rather than decorative
