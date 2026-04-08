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

Migration should preserve **semantic surfaces and state wiring**, not inherited screen geography.

That means:
- keep controller/state/event truth
- keep existing surface components where useful
- keep widget-ready semantic boundaries as they emerge
- do **not** assume current left/right/center placement is the correct long-term cockpit composition

The redesign should therefore recompose existing wired surfaces into a new cockpit grammar from first principles rather than renaming today's rails and calling it done.

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

## Standard-display design target

For standard display targets, Auspex should aim for a **live operator cockpit HUD**.

This is the intended feel for:
- desktop windows with normal or wide aspect ratios
- large attached monitors
- standard web targets with enough horizontal surface to support concurrent instrumentation

### What this means

The cockpit HUD target implies:
- a persistent global operational frame rather than page-like screen transitions
- bounded instrument panels layered on a stable structural field
- deployment and activity truth remaining globally legible
- dense but disciplined readouts
- state-driven liveness across the shell
- explicit ambient vs focal vs interruptive information hierarchy

### What this does not mean

The cockpit target does **not** justify:
- cinematic concept-art dashboards
- decorative rings or gauges without operator meaning
- glow-heavy sci-fi ornament saturation
- unreadably tiny labels used only for aesthetic density
- motion that exists for spectacle rather than state meaning

### Composition rule

The standard-display cockpit should always preserve the global truth spine:
- `AuspexPanel`
- `AttachedOmegonPanel`
- `DeploymentPanel`
- `ActivityPanel`

Secondary investigative and task surfaces (transcript, audit, work, graph, telemetry, dispatch detail) should compose around that spine instead of replacing it entirely.

### Liveness rule

The shell should feel live because:
- actor states change
- routes and authorities change
- lifecycle/freshness changes occur
- dispatches appear, progress, and complete
- telemetry and transcript state update

It should **not** feel live because of ambient decorative animation alone.

### Visual guardrails for standard displays

The standard-display cockpit should favor:
- dark structural field
- restrained dot-grid / seam-grid substrate
- crisp panel boundaries
- compact readout rails
- modest accent colors for meaning
- selective instrumentation
- text-first summaries augmented by geometry where justified

It should avoid:
- full-screen illustrative HUD art
- every panel becoming a spectacle widget
- replacing textual truth with purely graphical metaphors
- flattening all surfaces into equally bright visual noise

### Relationship to mobile and reduced displays

Mobile and reduced-surface layouts should inherit this same semantic system but trim it down through the reduced-surface policy.

Meaning:
- the standard-display cockpit is the full expression
- reduced displays are projections of the same system
- mobile is not a separate aesthetic product line

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

## Global truth surface stack

Before routed workspaces and secondary investigation panels, Auspex should establish a stable global truth stack that remains conceptually persistent across the shell.

### Layer 0 — canvas substrate

The substrate is the stable instrument-wall field:
- dark structural background
- subtle grid/seam language
- bounded depth cues
- no primary operational meaning encoded in decorative art

This layer provides stability, not explanation.

### Layer 1 — Auspex surface

This answers: **what this shell is**.

It should establish:
- Auspex identity
- version / channel / build lineage
- current UI mode / density class
- shell runtime context

### Layer 2 — Attached Omegon surface

This answers: **what our primary Omegon is right now**.

It should establish:
- currently attached/embedded/serve Omegon identity
- route/dispatcher truth
- verified control-plane state
- effective runtime role/profile/model/thinking tier
- health / degraded / stale status

### Layer 3 — Deployment surface

This answers: **what other Omegon instances this Auspex can see**.

It should establish:
- known deployment inventory
- serve-mode vs temporary worker classification
- live vs stale/lost/detached visibility
- ownership and placement context

### Layer 4 — Activity surface

This answers: **who is where doing what**.

It should establish the current distributed operational picture:
- which instances are active
- what task or stream each actor is bound to
- current lifecycle/urgency state
- what is blocked, running, idle, or recently completed

### Stack rule

These global truth surfaces should be considered semantically prior to secondary routed workspace content like graph, audit, and deeper work inspection.

They are not necessarily always rendered as one visible column, but they should remain the first conceptual layer the operator can recover when orienting inside the shell.

## UI surface to `auspex` tool-surface rule

Auspex must not become a presentation-only dashboard over Omegon.

### Canonical rule

Every operator-meaningful UI surface should have a corresponding machine-usable `auspex` tool/capability surface for the internal embedded/serve Omegon, subject to explicit policy gates.

If the human can meaningfully inspect or act on a supervisory surface, the internal Omegon should be able to inspect or request action through the same supervisory reality.

### Consequence

The internal embedded/serve Omegon should treat Auspex as a supervisory tool surface rather than only as a passive frontend.

This lets the internal Omegon:
- understand the current deployment picture
- understand route/dispatcher truth
- inspect activity and lifecycle state
- request or initiate supervised actions on behalf of the operator where policy allows

### Capability families

The top-level surface taxonomy should eventually correspond to `auspex` capability families such as:
- `auspex.identity.*`
- `auspex.attached.*`
- `auspex.deployment.*`
- `auspex.activity.*`
- `auspex.dispatch.*`
- `auspex.audit.*`
- `auspex.work.*`
- `auspex.telemetry.*`

This is not a requirement for one tool per widget. It is a requirement that the semantic UI surfaces map to semantic machine-usable surfaces.

### Access classes

Not every `auspex` surface implies unconstrained automation.

The model should distinguish at least:

#### 1. Read surfaces
Safe inspection of supervisory truth.

Examples:
- current attached Omegon summary
- deployment inventory
- lifecycle/freshness state
- current route truth
- activity summaries

#### 2. Request-action surfaces
The internal Omegon may request an action, but Auspex still validates and enforces policy.

Examples:
- route selection changes
- dispatcher profile/model switch requests
- temporary worker requests
- serve-mode worker requests
- focus/inspect transcript or audit targets

#### 3. Auspex-enforced surfaces
Actions or state transitions that remain shell-owned and are not delegated as unconstrained tool execution.

Examples:
- destructive cleanup/reap policy
- sensitive credential or binding transitions
- final lifecycle enforcement
- transport/binding integrity decisions

### Design implication

The global truth surfaces should be designed as dual-purpose supervisory surfaces:
- legible to the human operator
- queryable/actionable to the internal Omegon via the `auspex` tool surface

That shared semantic contract is what prevents Auspex from collapsing into “just another dashboard app.”

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
- `DeploymentInventorySurface` — overview-with-drill-down over all visible Omegon instances, including same-box long-lived serve workers
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
- `Chat` hosts transcript, composer, dispatch, and selected ambient deployment truth, but it remains only one possible occupant of the universal focus host
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

## Standard-display cockpit composition

The standard-display cockpit layout should be defined from first principles rather than from the current shell geography.

### Composition rule

Do not begin with:
- left rail
- right rail
- center pane
- bottom bar

Begin instead with:
- what truths must always remain visible
- what is focal vs ambient
- what is interruptive vs contextual
- what surfaces are globally persistent vs task-hosted

### Intended standard-display composition

A standard display should compose around these semantic regions:

#### 1. Identity spine
Persistent shell identity and mode truth.

Feeds primarily from:
- `AuspexPanel`
- global mode/version/build context

#### 2. Primary attachment truth region
The main operator-facing truth for the currently attached/embedded/serve Omegon.

Feeds primarily from:
- `AttachedOmegonPanel`
- route/dispatcher truth
- verified control-plane state

#### 3. Deployment overview region
The broader view of known Omegon instances and their roles/lifecycle states.

Feeds primarily from:
- `DeploymentPanel`
- lifecycle and serve/temporary classification surfaces

#### 4. Activity region
The current distributed operational picture.

Feeds primarily from:
- `ActivityPanel`
- temporary dispatches
- active task and stream summaries

### Center-anchored layout rule

The standard-display cockpit should be laid out **center-first**, not edge-first.

#### Canonical geometric rule

The center of the focus host / COP bay should remain pegged to the center of the usable canvas.

Other major regions should then resolve from offsets relative to that anchored center bay:
- top instrument rail offsets from the top shell edge and COP width
- right support/detail bay offsets from the right shell edge and COP boundary
- lower operator/composer tray offsets from the bottom shell edge and COP boundary
- auxiliary regions should resolve from the remaining shell-space after the COP anchor is established

#### Why this matters

If the layout is computed edge-first, resizing causes the main picture to drift and the shell starts to feel like a stretched dashboard.

If the layout is computed center-first:
- the operator's main picture remains visually stable
- supporting regions compress or expand around it
- wide/narrow resizing preserves the feeling of one anchored cockpit rather than a set of floating boxes

#### Responsive consequence

Across stretches and resizes:
- the COP bay should remain the visual anchor
- supporting regions should absorb most of the dimensional change
- the layout may compress side/support regions earlier than the center COP
- the center should preserve proportion and placement as long as the viewport allows

#### Implementation implication

Major shell geometry should be derived from:
- centered COP bounding rect
- top/bottom shell offsets
- support-bay offsets from COP and shell edges

rather than from independently edge-pinned cards.

#### 5. Focus host region
The currently selected detailed workspace or investigation host.

This region should be understood as the shell's **operator focus host** and, in practice, the application's shifting **common operating picture (COP)** surface.

It must not be permanently defined as "the chat area." Chat is only the default occupant.

Required behavior:
- default occupant may be the chat/transcript surface
- the composer may live beneath the focus host in its compact/default form
- active conversation can expand upward and temporarily dominate the focus host when the operator is in an active dialogue loop
- non-chat contexts may take over the focus host when they become the current operator priority

Examples of valid focus-host occupants:
- transcript / chat conversation
- audit investigation workspace
- graph / structure view
- work / progress view
- telemetry drilldown
- selected deployment or activity drilldown when that becomes the operator's main task

Design rule:
- the focus host is the universal COP display for the application, shaped by current operator context
- chat should not monopolize it by layout doctrine
- the center of the shell belongs to operator focus, not to one feature family

### Focus-host occupant contracts

Before final page formatting, Auspex should define **what belongs in the focus host** for each major operator context.

The focus host should generally be understood as having four possible layers:
- context header
- primary body
- supporting body rail
- action footer

Not every occupant needs every layer, but each occupant should have an explicit content contract.

#### 1. Chat COP

Role: default focus-host occupant.

**Must contain:**
- context header identifying chat/live conversation stance
- transcript as the primary body
- docked composer as the action footer
- dispatch context near or inside the footer
- blocking/setup interruption when prompting is unavailable

**May contain:**
- compact chat-status banner
- selected conversation/focus metadata

**Must not contain:**
- full deployment inventory
- deep provider auth cards
- generic session inspector dump

#### 2. Audit COP

Role: investigative focus host for "what happened?"

**Must contain:**
- context header for current audit scope
- audit result set as the primary body
- filter summary and selected-detail support rail
- actions for focus / jump / filter refinement

**Must not contain:**
- generic chat composer
- deployment summary duplication

#### 3. Deployment Drilldown COP

Role: selected-instance or selected-deployment focus host.

**Must contain:**
- context header naming the selected instance or deployment target
- selected instance/deployment detail as the primary body
- supporting rail for related instances, authority chain, bindings, or lifecycle context
- action footer for route / focus / inspect / reconnect-style actions where allowed

**Must not contain:**
- global deployment counts as the main body
- transcript spam by default

#### 4. Activity Drilldown COP

Role: per-actor/task focus host.

**Must contain:**
- context header naming the selected actor/task
- current task/activity detail body
- supporting metadata rail for owning instance, urgency, freshness, or work binding
- action footer for focus / cancel / jump-to-related-surface actions where allowed

**Must not contain:**
- raw fleet inventory
- generic shell metadata

#### 5. Graph / Work COP

Role: structure-oriented or progress-oriented focus host.

This occupant family remains valid but is lower priority than Chat, Audit, Deployment Drilldown, and Activity Drilldown for the current cockpit migration.

#### Occupant rule

The focus host should always answer:
**What is the operator currently focused on, and what is the minimum context + control set needed to act on it?**

### Focus-host occupant priority and anti-rugpull rule

The focus host needs an explicit shift policy so it can behave like a live COP without stealing control from the operator.

#### Priority classes

1. **Default occupant**
   - what appears when no stronger context is active
   - currently: `Chat COP`

2. **Operator-selected occupant**
   - chosen directly by the operator
   - examples: `Audit COP`, `Graph / Work COP`, explicit deployment or activity drilldown

3. **Action-follow occupant**
   - entered as the natural consequence of an operator action
   - examples: clicking a deployment preview chip, selecting an activity actor, focusing an audit event

4. **Escalation occupant**
   - may request attention because the system detects a materially important condition
   - examples: bootstrap failure, primary runtime failure, selected actor failure, blocking auth/control condition

#### Anti-rugpull rule

Auspex must **not rugpull the focus host away from the operator** during normal work.

That means:
- no automatic focus-host switch just because a new event arrives
- no automatic chat → audit / deployment / activity takeover for routine updates
- no replacing an operator-selected occupant with a different occupant unless the condition is truly interruptive

#### Allowed automatic focus changes

Automatic takeover is allowed only for clearly interruptive conditions, such as:
- bootstrap or attach failure
- primary runtime failure
- fatal incompatibility or control-plane loss
- an operator-triggered action whose whole purpose is to open a specific drilldown

Even then, the takeover should prefer:
- clear reason
- explicit source of the escalation
- obvious return path

#### Preferred escalation behavior

Before replacing the current focus host, prefer in this order:
1. top-spine state change
2. contextual-detail auto-expand
3. badge/notice requesting attention
4. explicit operator click-through into the new occupant
5. automatic focus-host takeover only if the condition is truly blocking or catastrophic

#### Restoration rule

If Auspex does take over the focus host automatically, it should preserve enough context to allow returning to the prior occupant when the interruptive condition is resolved or dismissed.

#### Occupant matrix (initial)

- `Chat COP`
  - default occupant: yes
  - operator-selectable: yes
  - automatic takeover: no
- `Audit COP`
  - default occupant: no
  - operator-selectable: yes
  - action-follow: yes
  - automatic takeover: no
- `Deployment Drilldown COP`
  - default occupant: no
  - operator-selectable: yes
  - action-follow: yes
  - automatic takeover: only for severe deployment/runtime failure
- `Activity Drilldown COP`
  - default occupant: no
  - operator-selectable: yes
  - action-follow: yes
  - automatic takeover: only for severe actor/task failure when operator intervention is urgently required
- `Graph / Work COP`
  - default occupant: no
  - operator-selectable: yes
  - automatic takeover: no

### Selection model

The cockpit needs one coherent selection model connecting:
- top-spine preview chips
- contextual-detail widgets
- focus-host occupants
- transcript/audit focus actions

Without this, the shell remains a collection of arranged surfaces instead of a coordinated instrument panel.

#### Canonical selection kinds

Selections should be modeled semantically, not as raw widget-local state.

Representative selection kinds:
- `SelectedDeploymentInstance(instance_id)`
- `SelectedActivityActor(actor_id)`
- `SelectedAuditEntry(block_id)`
- `SelectedTranscriptTarget(target)`
- `SelectedGraphNode(node_id)`
- `SelectedRoute(route_id)`
- `SelectedAuthority(instance_id | route_id)`

#### Source-of-selection

Every selection should also know where it came from:
- top truth spine preview chip
- contextual detail widget
- focus-host content body
- explicit workspace navigation
- escalation/notice path

This source matters for restoration, breadcrumbs, and anti-rugpull behavior.

#### Selection rule

A selection should not automatically imply a full focus-host takeover.

Preferred effects of selection, in order:
1. highlight the selected entity across relevant surfaces
2. open or emphasize the matching contextual-detail section
3. update local supporting rails / breadcrumbs
4. offer or allow explicit jump into the matching focus-host occupant
5. only replace the focus host immediately when the selection is clearly an action-follow drilldown

#### Preview-chip behavior

Top-spine preview chips should be treated as compact selectors.

- clicking a `DeploymentPanel` preview chip should select the instance and open or focus related deployment detail
- clicking an `ActivityPanel` preview chip should select the actor and open or focus related activity detail
- preview-chip selection may enter a drilldown COP when that click is clearly the operator's intent
- hover/focus states may reveal related detail, but should not change the focus host permanently

#### Detail-widget behavior

Contextual-detail widgets should respect the same selection model.

Examples:
- selecting a dispatcher option should update authority selection and related detail
- selecting a temporary dispatch should align the activity selection and offer transcript focus
- selecting a control-plane rollup row should align the deployment instance selection

#### Focus-host behavior

When a focus-host occupant is driven by a selection, it should expose:
- what entity is selected
- where the selection came from
- how to return to the prior occupant or clear the drilldown

#### Multi-surface alignment rule

A single selection should be allowed to illuminate multiple surfaces simultaneously.

Example:
- selecting a deployment instance may
  - highlight its preview chip in `DeploymentPanel`
  - open matching control-plane detail in the right column
  - optionally drive `Deployment Drilldown COP`

This is desirable. Selection is shared semantic context, not a one-widget local toggle.

#### Clear-selection rule

The operator must be able to clear or back out of a selection cleanly.

Clearing selection should:
- remove highlight/emphasis states
- collapse non-pinned drilldown surfaces where appropriate
- return focus-host occupancy to the prior/default context if the current occupant was purely selection-driven

### Operator interaction contract

This selection model is the canonical **operator contract** for cockpit interaction.

It should govern:
- implementation decisions
- future how-to guides
- debugging of visual behavior
- assessment of whether a UI element is behaving correctly

#### Contract statement

When an operator interacts with a visual element, the system should make the result legible and reversible.

That means every meaningful interactive visual element should answer four questions:
1. **What did I select?**
2. **What changed because of that selection?**
3. **Where did the related detail go?**
4. **How do I get back?**

If an interaction cannot answer those four questions, the interaction contract is broken.

#### Implementation rule

Interactive visual elements should not invent private navigation behavior.

They should map into the shared cockpit contract:
- select semantic entity
- highlight related surfaces
- open related detail
- optionally enter a drilldown occupant
- preserve a return path

#### Debugging rule

When debugging a visual element, do not start with CSS or animation.

Start with these checks:
1. did the correct semantic selection fire?
2. did the correct surfaces react?
3. did the focus host remain stable unless takeover was truly justified?
4. is the return/clear path intact?
5. only then inspect styling, transitions, or animation defects

#### Guide-writing rule

Future operator-facing guides and developer how-to docs should describe interactions in terms of this contract, not only in terms of page geometry.

Good guidance:
- "Clicking a deployment chip selects that instance, opens the related detail section, and lets you enter deployment drilldown if needed."

Bad guidance:
- "Click the small pill in the upper-right card to go to another page."

The contract is semantic first, visual second.

#### Visual language consequence

Animations, emphasis, hover states, chip highlights, disclosure changes, and focus-host transitions should all exist to clarify this contract.

They are successful only if they make selection, reaction, detail location, and return path easier to understand.

#### 6. Contextual detail region
Secondary detail and action surfaces tied to the current focus or selected entity.

Examples:
- dispatcher options
- instance detail
- route actions
- selected audit detail
- selected telemetry detail

### Surface contribution matrix

Existing surfaces should be mapped into this new composition by **responsibility**, not by old region.

Representative mapping:
- `render_attached_omegon_status_widget` → `AttachedOmegonPanel`
- dispatcher binding surfaces → `AttachedOmegonPanel` + contextual detail region
- lifecycle rollup → `DeploymentPanel`
- temporary dispatches → `ActivityPanel`
- transcript surfaces → focus host region
- audit surfaces → focus host region + contextual detail region
- graph/work surfaces → focus host region
- telemetry summaries → `DeploymentPanel` or contextual detail depending on scope

### Implementation consequence

During implementation, some of these compositions may temporarily reuse current shell containers. That is acceptable as an implementation stage.

But the design model itself should remain based on the new semantic composition, not on preserving legacy region names as first-class layout doctrine.

## Top-level UX panels and supervisory maps

Before deciding final visual treatment, the shell should define the top-level UX panels that establish operator orientation and the internal supervisory maps that feed them.

### Top-level UX panel stack

## Truth-panel specification

The standard-display cockpit uses a persistent top truth spine. These panels are not generic cards; they are global operator instruments.

### Global rule

Each truth panel must answer exactly one primary operator question. If a panel starts answering multiple questions, it should be split or demoted into a secondary/detail surface.

### Panel priority

1. `AttachedOmegonPanel` — strongest visual priority
2. `ActivityPanel` — second priority
3. `DeploymentPanel` — compact fleet overview
4. `AuspexPanel` — calm shell identity anchor

Visual equality between all four panels is acceptable for scaffolding, but not the intended long-term hierarchy.

### AuspexPanel

**Primary question:** What shell am I in, and what stance is it currently in?

**Required fields:**
- app identity (`Auspex`)
- version / release channel
- current workspace + mode summary
- shell health / stance summary

**Optional fields:**
- environment label
- operator profile / persona later
- reduced bootstrap notice if it becomes durable shell truth

**Must not contain:**
- deployment counts
- delegate activity detail
- attached-instance authority detail
- inventory/transport detail beyond shell posture

**Compact mode:** name + version + health only

### AttachedOmegonPanel

**Primary question:** What is my currently attached or primary Omegon authority?

**Required fields:**
- logical instance identity
- role
- profile
- freshness / verified-state summary

**Optional fields:**
- effective model
- endpoint / verified control-plane URL
- tier / thinking level if authoritative
- selected authority nuance if distinct from attachment

**Must not contain:**
- fleet counts
- multi-instance inventory
- broad activity feed
- generic shell metadata

**Ordering rule:**
1. logical identity
2. authority
3. freshness
4. binding metadata

Transport/route labels are secondary metadata, not the first line.

**Compact mode:** identity + freshness only

### DeploymentPanel

**Primary question:** What Omegon instances exist in my field of view, what kind are they, and which ones need attention?

This panel is not just a count badge. It is the compact **deployment field overview** for everything Auspex can currently see and reason about.

That field may include:
- remote or separately deployed Omegon instances
- long-lived serve-mode workers launched on the same box as Auspex
- long-lived detached services supervised by the same authority domain
- temporary task-bound dispatches when they materially affect field state

Same-box long-lived agents are valid deployment members. Locality does not exempt them from deployment visibility.

**Required fields:**
- total known count
- fresh/stale count summary
- compact instance roster, preview rail, or equivalent per-instance visibility

**Optional fields:**
- attached count
- serve-mode count
- temporary-dispatch count
- lost / abandoned count
- role-mix summary
- same-box vs remote placement hints when materially useful

**Must not contain:**
- attached primary detail
- transcript/audit activity
- deep per-instance control-plane fields inline by default

**Drill-down rule:**
- this panel must allow the operator to inspect specific instances in more detail
- it is an overview-with-drill-down surface, not merely a headline metric strip

**Compact mode:** headline counts plus minimal per-instance preview, not counts alone

### ActivityPanel

**Primary question:** Who is where doing what right now, and what needs operator attention first?

This panel is the compact **activity field overview** for work happening across the visible Omegon estate.

It should unify liveness across:
- the primary Omegon
- long-lived serve-mode workers
- temporary task-bound dispatches
- external/remote visible workers when they are within operator scope

This is not a generic status line and not a raw event feed.

**Required fields:**
- current activity state (idle/running/waiting/degraded/dispatching)
- active delegate / dispatch count
- compact actor roster, activity preview rail, or equivalent per-actor visibility

**Optional fields:**
- task/work summary
- urgency/freshness signal
- scope hint (primary vs external worker activity)
- serve-mode vs temporary activity breakdown
- blocked / waiting / failed actor counts

**Must not contain:**
- fleet inventory detail
- control-plane endpoints
- shell identity metadata
- provider auth/setup noise
- raw transcript or audit logs inline by default

**Drill-down rule:**
- this panel must allow the operator to inspect specific active or recently-active actors in more detail
- it is an overview-with-drill-down surface, not merely a headline status strip

**Compact mode:** headline state plus active count and minimal actor preview, not text status alone

### Future fifth panel: RouteAuthorityPanel

This panel is not required in the first cockpit scaffold, but it is likely to become necessary.

**Primary question:** Where will my next command go, and why?

Rationale:
- attached identity and command authority are not always the same thing
- multi-Omegon + serve-mode operation will eventually make authority selection more explicit than simple attachment state

This panel should only be promoted when the `AttachedOmegonPanel` starts carrying too much route/authority logic and becomes semantically muddy.

### Escalation rules

Truth panels may escalate visually when:
- the attached Omegon becomes stale/degraded/lost
- deployment freshness changes materially
- an authority change or route switch is pending/confirmed
- activity shifts from idle to active or degraded

Escalation should increase salience, not add decorative chrome.

### Reduced-surface policy

On narrow/square/mobile projections:
- `AttachedOmegonPanel` remains highest priority
- `ActivityPanel` remains visible or quickly accessible
- `DeploymentPanel` may collapse to a one-line summary
- `AuspexPanel` may compress to identity + version + shell health


These are the first persistent global truth panels from a UX perspective.

#### 1. `AuspexPanel`

Answers: **what this shell is**.

Responsibilities:
- Auspex identity
- version / release channel / build lineage
- current UI mode / density class
- shell runtime context
- current workspace/repo/session framing where appropriate

#### 2. `AttachedOmegonPanel`

Answers: **what our primary Omegon is right now**.

Responsibilities:
- primary attached/embedded/serve Omegon identity
- route/dispatcher truth
- verified control-plane state
- effective role/profile/model/thinking tier
- health, degraded, stale, or detached state

This is the default focal operator truth panel.

#### 3. `DeploymentPanel`

Answers: **what other Omegons this Auspex can see**.

Responsibilities:
- visible instance inventory
- live vs stale/lost/detached counts
- serve-mode vs temporary classification
- role and ownership context
- backend/placement context where useful

This is the broader deployment picture surrounding the primary attached Omegon.

#### 4. `ActivityPanel`

Answers: **who is where doing what**.

Responsibilities:
- current distributed actor roster
- active task or stream binding per actor
- blocked/running/idle/completed state
- urgency and freshness
- recent activity relevant to operator attention

This panel should become the shell's primary sense of liveness without degenerating into raw log spam. It is the field overview for active work, with drill-down into specific actors or tasks when needed.

### Relationship between panels

The intended reading order is:
1. `AuspexPanel` — what this shell is
2. `AttachedOmegonPanel` — what the operator is primarily attached to
3. `DeploymentPanel` — what else exists around that primary target
4. `ActivityPanel` — what is happening now across the visible system

Secondary surfaces such as transcript, audit, graph, work, and deep telemetry should compose underneath or beside this truth stack rather than replacing it.

## Supervisory state maps

These top-level panels should be projections over explicit internal supervisory maps.

### 1. Shell state map

Owns Auspex-local truth.

Examples:
- Auspex version/build/channel
- current workspace and mode
- current density class / layout class
- active layout preset
- shell/runtime placement context

Primary consumer:
- `AuspexPanel`

### 2. Deployment state map

Owns logical Omegon instance truth across the visible system.

Keyed by logical `instance_id` rather than transport binding.

Examples:
- identity
- role
- profile
- owner/session
- lifecycle state
- freshness / last seen
- backend placement
- verified control-plane binding
- available transport bindings
- serve-mode vs temporary classification
- selected/primary relationship

Primary consumers:
- `AttachedOmegonPanel`
- `DeploymentPanel`
- parts of `RouteSelectionSurface`, `DispatcherBindingSurface`, and `LifecycleRollupSurface`

### 3. Activity state map

Owns current work and stream activity across instances.

This is distinct from deployment identity. It answers what is happening now.

Examples:
- actor instance id
- actor role
- task/work binding
- current state
- source stream/channel
- last update time
- urgency
- operator visibility classification
- transcript/audit linkage where applicable

Primary consumer:
- `ActivityPanel`

### 4. Event ingestion / projection layer

The maps above should be fed by a normalized event/projection layer rather than letting widgets consume transport-specific events directly.

Expected source families include:
- transcript events
- tool events
- dispatch/delegate events
- lifecycle updates
- control-plane verification changes
- telemetry deltas
- route/authority changes
- future Styrene RPC updates

### Map rule

Widgets should consume these supervisory maps as projections. They should not invent parallel state caches or bind directly to one transport's ephemeral event vocabulary.

This keeps the live canvas truthful and compatible with future transport evolution.

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
