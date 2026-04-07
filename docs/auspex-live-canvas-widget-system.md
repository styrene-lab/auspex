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

## Identity and transport planning

The live canvas must plan for future Styrene RPC without letting transport details become the primary UI identity.

### Canonical identity rule

Widget identity and deployment rendering must anchor on **logical Omegon instance identity**, not on whichever transport is currently used to attach.

Primary identity should be derived from:
- `instance_id`
- `session_id` where applicable
- role (`primary-driver`, `supervised-child`, `detached-service`)
- profile
- workspace/project binding
- ownership/supervision state
- lifecycle freshness / last-seen state

### Transport is binding metadata, not identity

IPC, websocket, and future Styrene RPC are attachment/binding mechanisms.

They should be rendered as properties of a verified control-plane binding, not as the source of truth for who the instance is.

That means the UI should avoid treating these as canonical identity:
- websocket URL
- IPC socket path
- local port
- RPC endpoint address

These matter operationally, but only as binding metadata.

### Three identity layers for deployment widgets

#### 1. Logical instance identity

Stable across transport changes and reattachment:
- durable instance id
- role
- profile
- workspace ownership
- parent/child/session relationship

#### 2. Verified control-plane identity

The currently trusted authority proof for that logical instance:
- schema version
- Omegon version
- auth/token reference
- last verified time
- security mode
- verification freshness

#### 3. Transport binding set

The set of available concrete attach surfaces for the same logical instance.

Near-term examples:
- IPC binding
- websocket binding

Planned future example:
- Styrene RPC binding

A single instance may expose multiple bindings over time or simultaneously. The canvas should still render it as **one logical instance**.

### Styrene RPC planning rule

Future Styrene RPC adoption should slot into the transport binding layer without forcing a rename of deployment widgets or a rewrite of instance identity semantics.

Consequences:
- route selection should target logical authority first, transport second
- attached/deployment widgets should show transport as a verified binding property
- switching a binding from websocket or IPC to Styrene RPC should not change the rendered logical identity of the worker
- saved layouts and widget identity should not depend on transport-local endpoint strings

### Deployment-first rendering implication

For the primary deployment widgets, the information order should be:
1. who this instance is
2. what authority/role it has
3. what lifecycle state it is in
4. how it is currently bound

Not the reverse.

This keeps the canvas truthful even as the transport layer evolves.

## State and data ownership

The live canvas layout is a **presentation system**, not a new state owner.

### Ownership rule

`AppController` remains the owner of session/runtime/telemetry/route truth.

Widgets consume controller-projected data. They do not invent parallel caches or alternative state derivations.

### Consequence

A widget should be hideable, movable, or collapsible without changing the underlying application state model.

This preserves the rule from [[screen-bindings]]: one underlying state/cache, multiple projections.

## Reduced-surface policy

Reduced display surface should default to reduced cognitive surface.

### Canonical rule

Mobile and other reduced-surface layouts should enter **simple-mode projections first** unless the operator explicitly opts into expanded or power-density views.

This preserves the product rule from [[vision]]: one backend contract, multiple UI projections, with low-cognitive-load defaults and explicit power expansion.

### Default display-density policy

#### Wide desktop / wide web
- power-user composition is allowed by default
- multiple concurrent surfaces may remain visible
- persistent deployment/investigation/inspector surfaces are acceptable

#### Narrow desktop / tablet / square layouts
- default should bias toward reduced simultaneous visibility
- side inspectors should collapse into trays, drawers, sheets, or secondary stacks
- the system should prefer simpler projections before forcing dense layouts into cramped geometry

#### Mobile / portrait-first layouts
- default to simple-mode-first projection
- keep transcript, current activity, route/attachment truth, and composer primary
- require explicit operator intent to expose denser power surfaces

### Mobile display levels

The mobile composition should support at least three display levels:

#### 1. Mobile Simple
Default projection.

Visible by default:
- attached Omegon status
- current route / dispatcher truth
- transcript
- current activity / run state
- composer / action surface
- minimal provider readiness
- compact lifecycle/degraded warnings when actionable

#### 2. Mobile Expanded
Operator-requested expanded summary.

May add:
- deployment summary cards
- temporary dispatch summary
- compact telemetry
- compact audit access
- deeper route/dispatcher details

#### 3. Mobile Power
Explicit override only.

Allows:
- denser surface switching
- more inspector-style detail
- power-user investigation/workspace surfaces

But still must remain mobile-composed rather than forcing the full desktop wall into portrait.

### Surface visibility policy

Each surface should eventually declare default visibility by context, for example:
- `simple_default_visibility`
- `power_default_visibility`
- `mobile_default_visibility`
- `escalation_policy`

Examples:
- `AttachedOmegonStatusSurface` → visible in simple, power, and mobile
- `DeploymentInventorySurface` → hidden in simple, visible in power, collapsed/expandable on mobile
- `TemporaryDispatchesSurface` → escalates when active; summary-first on mobile
- `GraphOverviewSurface` → power-only by default; explicit override on mobile

### Escalation rule

Reduced-surface layouts may temporarily surface denser panels when the state becomes actionable or risky.

Examples:
- degraded attachment state
- long-running active dispatches
- lifecycle freshness loss
- route/authority changes that affect operator actions

This allows mobile to remain simple-first without hiding critical state when it matters.

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

## Surface inventory before visual treatment

Before deciding final widget visuals or canvas ornamentation, Auspex needs a complete surface taxonomy.

The purpose of this section is to define **what surfaces exist** and **what operator question each surface answers**, independent of final layout styling.

### Surface families

The live canvas should think in surface families rather than only in current routed screens.

#### 1. Deployment surfaces

These answer: **what Omegon instances exist, which one am I attached to, and what authority do they have?**

Core deployment surfaces:
- `AttachedOmegonStatusSurface`
- `RouteSelectionSurface`
- `DispatcherBindingSurface`
- `DeploymentInventorySurface`
- `LifecycleRollupSurface`
- `ServeModeWorkersSurface`
- `TemporaryDispatchesSurface`
- `RuntimeBackendPlacementSurface`

#### 2. Investigation surfaces

These answer: **what happened, what is happening, and how do I inspect it?**

Core investigation surfaces:
- `TranscriptSurface`
- `ToolActivitySurface`
- `AuditSummarySurface`
- `AuditFiltersSurface`
- `AuditResultsSurface`
- `AuditDetailSurface`
- `TelemetryDrilldownSurface`

#### 3. Workspace / reasoning surfaces

These answer: **what work is active, what structure is known, and what progress is blocked?**

Core workspace surfaces:
- `WorkContextSurface`
- `FocusedNodeSurface`
- `ImplementingNodesSurface`
- `ActionableNodesSurface`
- `GraphOverviewSurface`
- `OpenSpecSummarySurface`
- `CleaveSummarySurface`

#### 4. Operator control surfaces

These answer: **what can the operator do right now, and where will that action go?**

Core control surfaces:
- `ComposerSurface`
- `DispatchContextSurface`
- `ProviderAuthSurface`
- `RouteActionSurface`
- `InstanceActionSurface`
- `LayoutEditSurface` (future, explicit mode only)

#### 5. Ambient shell surfaces

These answer: **where am I, what mode am I in, and what global state must remain persistent?**

Core ambient shell surfaces:
- `TopChromeSurface`
- `IdentityAnchorSurface`
- `WorkspaceTabsSurface`
- `GlobalStatusSurface`
- `BottomInstrumentationSurface`
- `ReservedGlobalApertureSurface`

### Canonical operator questions per family

#### Deployment
- Which Omegon instance am I speaking to right now?
- Is it reachable, verified, and healthy?
- How many other instances are out there?
- Which ones are durable serve-mode workers versus temporary task workers?
- Which backend/placement is each one running on?

#### Investigation
- What just happened?
- What is streaming right now?
- Which event/tool/telemetry change matters?
- How do I focus related transcript or audit evidence?

#### Workspace / reasoning
- What work item is active?
- What design/spec/progress state is currently relevant?
- What is blocked, implementing, or actionable?

#### Operator control
- What action can I take now?
- Which instance or route will receive it?
- Is the action blocked by transport, provider, or lifecycle state?

#### Ambient shell
- What repo/session/workspace am I in?
- What UI mode am I in?
- What global status needs to remain visible without drilling in?

### Screen-model reconciliation

The canvas taxonomy should coexist with the existing routed workspace model.

Current routed workspaces still exist:
- Chat
- Session
- Audit
- Scribe
- Graph
- Work

But those routed workspaces should increasingly be understood as **surface hosts** rather than monolithic screens.

Examples:
- `Session` currently hosts deployment and inspection surfaces
- `Audit` currently hosts investigation surfaces
- `Chat` hosts transcript, composer, dispatch, and selected ambient deployment truth
- `Graph` hosts structure-oriented workspace surfaces
- `Work` hosts progress-oriented workspace surfaces

### Priority order for surface definition

The order for defining surfaces should be:
1. deployment surfaces
2. operator control surfaces
3. investigation surfaces
4. workspace / reasoning surfaces
5. ambient shell refinements

This preserves the product rule that Auspex is first an operator shell for real Omegon deployments, not a generic dashboard.

### Display-independence rule

Every surface definition should be valid before deciding:
- exact zone placement
- exact widget size
- exact styling treatment
- whether it becomes a compact card, full panel, drawer, or overlay

In other words: define the semantic surface first, then decide how it is rendered.

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
