+++
id = "auspex-visual-design"
name = "auspex-visual-design"
description = "High-ROI visual hierarchy, density, alignment, and application-shell rules for Auspex operator and assistant surfaces"
tags = ["auspex", "visual-design", "ui", "layout", "chat", "dashboard", "dioxus", "arwes"]
aliases = ["auspex-design", "visual-design", "assistant-ui", "operator-ui"]
activation = "domain_detected"
profile = ["coding", "design"]
project_signals = ["Dioxus.toml", "src/app.rs", "assets/main.css"]
+++

# Auspex Visual Design

Use this skill whenever designing, implementing, or reviewing an Auspex screen, especially assistant chat, agent dashboards, runtime controls, telemetry, transcripts, and operator-console layouts.

This skill governs **what receives space and attention**. The companion `dioxus-arwes-ui` skill governs implementation integrity, backend wiring, containment, and Dioxus/Arwes mechanics. Apply both when relevant.

## Prime directive

Design around the operator's primary task, not around every datum the backend can expose.

For a single-agent assistant surface:

> Chat is the application. Runtime state, telemetry, transport, and configuration are context available on demand.

Do not render a dashboard above chat and call the result a chat experience. If conversation is the primary task, the transcript owns the dominant region and the composer remains visible.

## High-ROI rules

Apply these before decorative polish. If these are wrong, adjusting shadows, colors, or spacing will not rescue the screen.

### 1. One screen, one focal task

- Identify the primary operator task before laying out the page.
- Give one region dominant area and contrast.
- Treat secondary data as a compact summary, side rail, drawer, tab, or disclosure.
- Do not stack several primary surfaces—dashboard, telemetry, configuration, transcript, and composer—in one document flow.
- Do not create a second full screen inside the first through another large heading and frame.

For assistant chat, default to:

```text
application chrome
compact target and status bar
transcript: minmax(0, 1fr), sole scroll owner
composer: always visible
```

### 2. Use the viewport as an application shell

- Prefer a bounded grid or flex shell over a vertically growing report page.
- Reserve explicit rows for headers, compact context, the scroll body, and actions.
- Keep the primary action reachable without page scrolling.
- Give the transcript or principal work region the remaining height.
- Put expanded diagnostics and configuration in a drawer, rail, modal, or disclosure rather than above the work surface.
- Never use arbitrary `min-height: calc(100vh - ...)` values in multiple nested descendants to simulate containment. Establish one owning shell.

### 3. Eliminate accidental dead space

- Empty space is useful only when it reinforces hierarchy or reading comfort.
- A half-width dashboard inside a full-width frame is a layout bug unless another intentional column exists.
- Either fill the established content width, center a deliberately bounded reading column, or assign the remaining width to a real side rail.
- Do not compress labels and controls while leaving a featureless region beside them.
- At wide breakpoints, inspect whether major panels align to the same content grid.

### 4. Establish stable alignment guides

- Define a small set of shared left and right edges for the screen.
- Align the target header, transcript, composer, and principal status surface to those guides.
- Avoid the indentation staircase caused by deeply nested framed containers.
- Major sections on the same screen should not switch casually between unrelated widths.
- Align labels and values with a deliberate grid; do not leave values clustered at one edge of a wide card.
- Match composer width to the transcript reading lane unless a wider composer has a specific functional reason.

### 5. Reduce box soup

Use at most three visible containment levels:

1. application region,
2. primary pane,
3. interactive or exceptional subcomponent.

Rules:

- Borders communicate grouping, selection, or state; they are not default decoration for every datum.
- Do not put a frame around a frame around a frame.
- Prefer spacing, typography, and subtle surface contrast for ordinary grouping.
- Reserve strong borders for focus, selection, warning, or major pane boundaries.
- Content should be easier to perceive than its outlines.

### 6. Match visible area to information value

- Zero, unknown, pending, and empty values should collapse rather than occupy full telemetry cards.
- `0 turns · 0 tools · context unavailable` is usually better than three large panels.
- Healthy transport is a small status indicator; full endpoint/protocol details belong in diagnostics.
- Show detailed telemetry when active, exceptional, selected, or explicitly requested.
- The interface should become quieter when there is less to report.
- Never equate more visible metadata with more operational clarity.

### 7. Progressive disclosure is the default

Keep these compact or collapsed on a conversational surface:

- identity/profile files,
- backend and workspace details,
- transport endpoints and protocols,
- routine connection events,
- telemetry with no activity,
- advanced next-turn controls,
- raw runtime diagnostics.

Provide clear controls such as `Runtime details`, `Configure next turn`, or `Connection diagnostics`. Do not use an unexplained glyph where the action changes scope, opens another product surface, or navigates away.

### 8. Reconcile requested and observed state together

Auspex frequently has distinct states:

- requested next-turn settings,
- observed runtime settings,
- transport connectivity,
- active conversation state,
- unknown or not-yet-reported values.

Do not scatter these across distant panels. Present reconciliation in one component or table:

```text
Setting     Requested       Observed       State
Model       GPT-5.5         Not reported   Unknown
Provider    OpenAI Codex    Anthropic      Mismatch
Thinking    Medium          Not reported   Pending
```

Rules:

- Label requested, observed, unknown, mismatch, and pending explicitly.
- Connected transport does not imply runtime settings are observed.
- A selected next-turn value does not imply the active runtime already uses it.
- Do not let technically truthful values appear contradictory because lifecycle categories are hidden.
- Action labels must describe their effect. Prefer `Apply next turn` or `Refresh observed state` over ambiguous `Sync`.

## Assistant and transcript rules

### Make conversation dominant

- The transcript is the largest and calmest region.
- The composer remains visible at the bottom of the shell.
- The transcript owns scrolling; the whole page should not scroll during ordinary conversation.
- Keep the target/status header compact.
- Runtime controls must not push the current conversation below the fold.

### Keep infrastructure out of the conversation lane

Routine events such as these are diagnostics, not messages:

- connected to event stream,
- attached to control plane,
- websocket endpoint selected,
- transport ready.

Show a compact connection indicator. Put detailed events in a diagnostics disclosure. Surface a system notice in the transcript only when it changes what the operator must know or do.

### Empty states must orient the operator

When there are no conversational turns, show:

- what agent/workspace is active,
- what the agent can do,
- important limitations or degraded state,
- a small number of useful first actions or prompts.

Do not fill an empty transcript with plumbing messages. Empty space should teach the next action.

### Composer discipline

- Size the composer for the likely input; expand on focus or content when useful.
- Keep input and Send action visually cohesive.
- Make disabled state unmistakable and explain blocking conditions when relevant.
- Summarize active next-turn configuration near the composer in one quiet line.
- Keep advanced settings behind `Configure` unless changing them is the screen's primary task.
- Do not use an enormous full-width command field when the transcript uses a narrow reading lane.

## Information hierarchy

### Titles and navigation

- Use one screen title, not a product heading plus a second embedded-screen heading.
- Avoid repeating the selected agent name in the page title, section title, composer placeholder, and status copy.
- A page title should identify the operator's current place, not restate implementation ownership.
- Management actions such as `Add agent` do not belong in the dominant action group of a single-agent conversation unless agent management is the current task.
- Separate routine utility, navigation, creation, and destructive actions by placement and emphasis.

### Typography

- Use sentence or title case for screen and section names.
- Reserve uppercase monospace with tracking for short instrumentation labels.
- Do not uppercase ordinary values, model names, help copy, and every button.
- Wide letter spacing is an accent, not the default voice.
- Secondary text must remain legible: avoid combining tiny size, low contrast, uppercase, and wide tracking.
- Humanize implementation tokens for primary UI; show raw identifiers only where operationally useful.
- Use monospace intentionally for IDs, paths, endpoints, hashes, and code—not for all prose.

### Color semantics

Use theme tokens, but preserve stable meaning:

- cyan/primary: selected, active, or healthy connectivity,
- green: accepted, complete, or successful,
- amber: pending, degraded, unknown, or awaiting reconciliation,
- red: failed, rejected, destructive, or unsafe,
- muted neutral: unavailable, historical, secondary, or inactive.

Do not use cyan for branding, every border, every metric, all headings, and all controls simultaneously. If everything glows, state cannot be read.

## Density budget

Before accepting a screen, count its simultaneously visible layers and concepts.

A default assistant view should usually expose only:

- target identity,
- connection/availability state,
- transcript,
- compact active configuration summary,
- composer and immediate actions.

Everything else must earn permanent space through one of these tests:

1. The operator needs it to complete the primary task now.
2. It is exceptional and requires attention now.
3. It changes frequently enough that hiding it causes real operational cost.
4. The operator explicitly opened a detail or management mode.

If none applies, collapse it.

## Responsive expectations

Test at minimum:

- typical laptop viewport,
- wide desktop viewport,
- narrow window/mobile breakpoint,
- empty transcript,
- long transcript,
- long identifiers/URLs,
- degraded or blocking state.

Wide screens should not merely stretch outer frames around narrow content. Narrow screens should preserve the primary task and move secondary context behind disclosures rather than stacking every panel above it.

## Screenshot review protocol

When given a screenshot, inspect in this order:

1. **Primary task:** What action appears dominant? Is it the intended one?
2. **Viewport use:** Is the primary action visible? Who owns scrolling?
3. **Geometry:** Mark major left/right guides and unexplained dead space.
4. **Hierarchy:** Count competing headings and visually primary regions.
5. **Density:** Identify permanent information with low current value.
6. **Containment:** Count nested frames and competing borders.
7. **State truth:** Separate requested, observed, connected, pending, and unknown.
8. **Typography:** Check uppercase, tracking, contrast, and raw implementation tokens.
9. **Affordances:** Find ambiguous icons and vague action labels.
10. **Responsive risk:** Predict what fails on an ordinary laptop and with long content.

Fix structural issues before polish.

## Anti-patterns to reject

- Dashboard above chat on a chat-first screen.
- Full-page document flow for an interactive console.
- Multiple competing scroll owners.
- Half-width content with unexplained dead space.
- Every datum inside a bordered card.
- Several unrelated content widths on one page.
- Permanently expanded diagnostics and advanced settings.
- Healthy connection events rendered as conversation turns.
- Large zero-state telemetry panels.
- Requested and observed state shown far apart.
- Ambiguous actions such as unlabeled launch arrows or generic `Sync`.
- Uppercase, tracked monospace applied to nearly all text.
- Styling polish performed before fixing hierarchy and geometry.
- Hiding overflow instead of establishing correct containment.

## Default implementation sequence

1. State the primary operator task in one sentence.
2. Sketch the shell as header / dominant work region / persistent actions.
3. Classify every visible datum as primary, contextual, diagnostic, or advanced.
4. Move contextual and diagnostic information behind progressive disclosure.
5. Establish shared alignment guides and one scroll owner.
6. Consolidate requested-versus-observed runtime truth.
7. Implement empty, active, degraded, and long-content states.
8. Review a real screenshot at laptop and wide-desktop sizes.
9. Only then tune typography, color, border strength, and decorative effects.
10. Validate behavior and visual containment before declaring the surface complete.

## Completion checklist

- [ ] The intended operator task is visibly dominant.
- [ ] The primary action is reachable without page scrolling.
- [ ] One element owns scrolling in the main work region.
- [ ] Wide-screen space is intentionally allocated.
- [ ] Major regions share stable alignment guides.
- [ ] There are no unnecessary nested frames.
- [ ] Low-value empty/unknown telemetry is compact.
- [ ] Diagnostics and advanced settings use progressive disclosure.
- [ ] Requested and observed runtime state are reconciled together.
- [ ] Routine transport events are absent from the transcript.
- [ ] The empty state explains a useful next action.
- [ ] The composer is cohesive, bounded, and visibly actionable.
- [ ] Action labels state their effect.
- [ ] Typography and color preserve semantic hierarchy.
- [ ] Laptop, wide, narrow, and long-content states were reviewed.
