+++
id = "dioxus-arwes-ui"
name = "dioxus-arwes-ui"
description = "Guardrails for Dioxus + Arwes frontend UI work in Auspex/Omegon surfaces"
tags = ["dioxus", "arwes", "frontend", "ui", "rust"]
aliases = ["dioxus", "arwes", "frontend-ui", "secundus-ui"]
activation = "domain_detected"
profile = ["coding", "design"]
project_signals = ["Dioxus.toml", "site/package.json", "src/**/*.rs", "site/**/*.ts", "site/**/*.tsx"]
+++

# Dioxus + Arwes UI Skill

Use this skill for frontend work involving Dioxus, Arwes-themed components, Auspex/Omegon web surfaces, console dashboards, transcript panes, action cards, surface streams, and backend-wired UI state.

The recurring failure mode is treating UI work as isolated markup. It is not. In this project, UI correctness depends on the data contract, async stream lifecycle, layout containment, and theme primitives all staying aligned.

## Operating posture

- Read before editing. Identify the owning component, the normalized view model, and the backend DTO/fixture before changing presentation.
- Make the smallest end-to-end slice that preserves the contract: DTO/fixture → normalization → component → styles → tests.
- Do not invent local UI-only state when backend or normalized surface state already owns the truth.
- Prefer boring containment and explicit state over clever visual layering.
- Do not declare a UI change complete until it has been checked against at least one real fixture/snapshot or backend-shaped envelope.

## First checks on every Dioxus/Arwes UI task

1. Locate the surface/component being changed.
2. Locate its data source:
   - native surface stream envelope
   - backend action response
   - fixture/snapshot
   - normalized view model
3. Locate the style boundary:
   - component-scoped CSS/module
   - Arwes theme token usage
   - parent shell/grid constraints
4. Determine whether the change is:
   - visual-only
   - contract/DTO-facing
   - async/action lifecycle-facing
   - layout/containment-facing

If any of those four points are unknown, scout before editing.

## Dioxus rules

- Keep component props typed and narrow. Avoid passing broad backend DTOs directly into deep visual components.
- Derive display models at the boundary, not inside every leaf component.
- Do not duplicate backend state into local component state unless it is genuinely ephemeral UI state such as hover, disclosure, focus, or optimistic pending status.
- For action buttons/cards, model the lifecycle explicitly: idle → submitting → accepted/rejected/error → refreshed.
- Never key dynamic lists by index when backend IDs, segment IDs, action IDs, request IDs, or surface IDs exist.
- Keep conditional rendering structurally stable where possible. Avoid layout jumps from swapping entire containers when a small status region would suffice.
- Treat missing backend fields as a state to render intentionally, not as a reason to unwrap or silently omit critical affordances.

## Arwes/theme rules

- Use Arwes/theme tokens and existing primitives first. Do not fight the theme with ad-hoc hex values or one-off shadows.
- Preserve the high-contrast console aesthetic: readable foreground, visible borders, clear active/inactive states.
- Motion/effects are secondary to operational clarity. If animation obscures status, remove or reduce it.
- Keep glow, scanline, and frame effects inside their component bounds. They must not leak into adjacent panes or intercept pointer events.
- Use semantic color meaning consistently:
  - primary/cyan: active link, selected surface, healthy connection
  - green: accepted/done/success
  - amber/orange: pending/degraded/waiting
  - red: rejected/error/destructive
  - muted: unavailable/history/inactive

## Layout and containment rules

This is where we keep tripping. Apply these before touching visuals:

- Every pane in a split/shell layout needs explicit min-size behavior. In CSS grid/flex contexts, use `min-width: 0` and `min-height: 0` on containers that hold scrollable content.
- Exactly one element should own scrolling for a region. Avoid nested competing `overflow: auto` unless deliberately building an inner scroll area.
- Transcript/log regions should be column layouts with a bounded scroll body, not full-page overflow hacks.
- Top bars, command bars, and status strips must not overlap scroll bodies. Reserve their height in the layout instead of absolute-positioning over content.
- Avoid `position: absolute` for primary layout. Use it only for decorative layers with `pointer-events: none`.
- For long tokens, IDs, paths, model names, and URLs, set wrapping policy intentionally: `overflow-wrap: anywhere`, truncation, or monospace clipped display.
- Test empty, loading, error, short-content, and long-content states. Most overlap bugs appear in long transcript/action/error states.
- If a cell/pane visually overflows, fix the parent containment; do not hide the symptom with random `overflow: hidden` unless clipping is the intended UX.

## Backend wiring rules

- Match the native/backend contract exactly. Do not rename fields in component code to make them look nicer; normalize once at the boundary.
- Preserve canonical backend links such as stream/action endpoints in the normalized view model when they are relevant to the UI.
- Surface stream handling must tolerate:
  - initial snapshot
  - incremental update
  - reconnect/resubscribe
  - stale/closed stream
  - malformed or unknown envelope variants
- Composer/action flows must refresh or reconcile the affected surface after backend acceptance. Do not assume optimistic UI is authoritative.
- Permission/approval cards must carry stable backend identifiers such as `request_id`; labels are not identifiers.
- Display transport state visibly when backend state is central to the task: connected, connecting, stale, error, mock/fallback.

## Class coverage and CSS authority rules

These exist because we shipped an unstyled provenance block (giant wrapped version text) and a button with three conflicting rule blocks (glyph geometry forced onto a text label).

- **No unstyled operational classes.** Every class name emitted from RSX must have a corresponding CSS rule. Before declaring a new component done, grep the stylesheet for each class you introduced. An element with no rule inherits body-scale typography — inside a narrow rail or strip this renders as giant wrapped text. If an element is intentionally unstyled, say so in a comment next to the RSX.
- **One authoritative rule block per component class.** When restyling an existing class, edit its original rule block in place. Never append a later "override" block further down the stylesheet — especially not one using `!important` to win. Three definitions of the same selector at different line numbers is a bug even when the cascade currently resolves the way you want. Grep for the class first: `grep -n "class-name" assets/main.css`. If multiple blocks exist, consolidate before changing anything.
- **Glyph buttons and text buttons are different components.** Fixed-width square geometry (`width: 2.35rem; place-items: center`) is only valid for single-glyph buttons (×, ⤢, ⚙). A button whose label is a word ("Focus", "Refresh") must size from its content: `padding-inline` + `min-height`, never fixed width. If a button can hold either, split the class or use a modifier.
- **Metadata/provenance text has a standard treatment.** Build stamps, versions, pids, timestamps, hashes: mono font, `0.5–0.6rem`, muted color, explicit wrap policy (`overflow-wrap: anywhere` for hashes/versions in narrow columns), no text-transform. Never let these inherit heading or body scale.

## Testing and validation

For non-trivial UI work, include at least one of:

- contract/fixture test for backend-shaped envelope parsing
- normalization test for display model derivation
- component/render test if the project has harness support
- snapshot/fixture update showing the new state
- focused manual run with the real dev server if rendering behavior is the risk

Rust project defaults still apply:

- `cargo check` for type checking
- `cargo clippy` for lints when touching logic-heavy Rust
- `cargo test` for tests

For web/frontend packages, use the project scripts if present rather than inventing commands. Inspect `package.json`, `Dioxus.toml`, `dx` config, or justfile before running.

## Review checklist before completion

- [ ] Component reads from the correct normalized data source.
- [ ] Backend identifiers are preserved through DTO → model → UI action.
- [ ] Loading/empty/error/degraded states render intentionally.
- [ ] Long transcript/action/error content cannot force shell overflow.
- [ ] Scroll ownership is clear and bounded.
- [ ] Top/status/action bars do not overlap scroll bodies.
- [ ] Theme tokens are used instead of ad-hoc colors.
- [ ] Tests or fixtures cover the contract/state touched.
- [ ] Every class emitted in RSX has a CSS rule (grep the stylesheet).
- [ ] No selector is defined in more than one rule block.
- [ ] Text-labeled buttons size from content; only glyph buttons get fixed square geometry.
- [ ] Validation command was run or the reason it could not run is stated.

## Anti-patterns to reject

- Styling around a data-shape mismatch instead of fixing the normalizer/DTO.
- Adding another local `use_signal`/state holder for backend-owned state.
- Keying backend lists by array index.
- Absolute-positioning operational UI to make it "fit".
- Sprinkling `overflow: hidden` on ancestors without identifying the scroll owner.
- Replacing an Arwes/theme primitive with custom markup for a minor visual tweak.
- Treating mock data success as proof that native backend envelopes work.
- Declaring layout fixed without testing long content.

## Default implementation sequence

1. Scout: component, data contract, style boundary, fixture/test.
2. Patch contract/normalizer first if needed.
3. Patch component with typed props and stable IDs.
4. Patch CSS/layout containment with explicit scroll ownership.
5. Add/update fixture or test.
6. Validate narrowly.
7. Summarize changed files, validation, and any remaining visual risk.
