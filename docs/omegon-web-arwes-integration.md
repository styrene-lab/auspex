---
title: Omegon Web Arwes Integration
status: exploring
tags: [omegon-web, arwes, dioxus, spa, ui]
---

# Omegon Web Arwes Integration

## Decision

Omegon Web should use **Arwes** for the decorative sci-fi UI chrome while remaining a **Dioxus SPA**. Dioxus owns application state, data providers, dummy/live backend contracts, and event handling. Arwes owns the visual chrome layer: frames, panels, animated surfaces, text effects, background effects, and bleep/sound affordances where appropriate.

## Integration posture

Do not rewrite Arwes visuals in Rust/CSS if an Arwes package can provide the chrome. The first implementation should prove a thin Dioxus-to-Arwes bridge with dummy-backed surface data, then keep replacing dummy providers with Omegon Web backend calls as `omegon-secundus` lands them.

## Candidate integration routes

### Route A — Arwes vanilla packages + Dioxus DOM

Use Arwes framework primitives that do not require React where possible, especially:

- `@arwes/frames`
- `@arwes/animated`
- `@arwes/animator`
- `@arwes/bgs`
- `@arwes/text`
- `@arwes/theme`

Dioxus renders normal DOM nodes with stable IDs/classes. Small JS modules attach Arwes frame/background/text effects to those DOM nodes after render.

Pros:

- Dioxus remains the only component runtime.
- Lower risk than running a nested React tree.
- Good fit for chrome primitives: frames, backgrounds, animation, text effects.

Cons:

- Some Arwes React conveniences may not be available directly.
- Requires authoring a small JS adapter layer.

### Route B — React/Arwes islands mounted inside Dioxus

Bundle React + Arwes React components as browser JS islands and mount them into Dioxus-owned container nodes. Dioxus passes serialized props/events through `wasm-bindgen`/`web-sys` or DOM custom events.

Pros:

- Uses Arwes React componentry directly.
- Fastest path if the desired chrome exists primarily as React components.

Cons:

- Two UI runtimes on one page.
- Must manage mount/unmount/re-render lifecycle explicitly.
- React Strict Mode must not be enabled for Arwes React.
- Requires a JS bundling step or checked-in bundle for the Dioxus web build.

### Route C — Web components wrapper

Create custom elements such as `<omegon-arwes-frame>`, implemented in JS/React/Arwes, then render those tags from Dioxus.

Pros:

- Clean boundary: Dioxus treats Arwes chrome as DOM elements.
- Better long-term isolation than ad hoc React islands.
- Supports progressive replacement or fallback.

Cons:

- Slightly more upfront wrapper work.
- Event/prop serialization must be deliberately designed.

## Recommended MVP route

Start with **Route C backed by Route B internally**:

1. Add a small `web-ui`/vendor JS bundle that imports React and Arwes.
2. Register custom elements for Arwes chrome primitives.
3. Render those custom elements from Dioxus components.
4. Keep Dioxus state and mock backend data in Rust.
5. Use DOM custom events for user actions that originate inside Arwes wrappers.

This satisfies the product direction: the app remains Dioxus, but Arwes provides the chrome instead of cloning its visual language.

## Initial custom elements

- `<omegon-arwes-app-shell>` — global background/chrome wrapper.
- `<omegon-arwes-panel>` — framed panel for top bar, transcript container, rail cards, approval cards.
- `<omegon-arwes-button>` — decorative button shell around Dioxus-provided label/action.
- `<omegon-arwes-status-pill>` — animated state pill for live/thinking/waiting/degraded.
- `<omegon-arwes-text>` — optional animated text treatment for headings/status.
- `<omegon-arwes-bg>` — background grid/noise/passive animation.

## Dioxus component mapping

| Omegon Web component | Arwes chrome |
|---|---|
| App shell | background effects + outer frame |
| TopBar | framed status strip, animated status pills |
| ConversationStream | framed transcript viewport; individual tool/approval cards as panels |
| ToolCard | compact Arwes panel, expandable details rendered by Dioxus |
| ApprovalCard | high-emphasis Arwes panel with warning color/animation |
| ContextRail | stacked Arwes panels |
| Composer | framed input dock; Dioxus owns textarea |
| CommandPalette | Arwes modal/panel wrapper |
| SettingsDrawer | Arwes framed drawer |

## Build notes for current Auspex stack

Current project facts:

- Dioxus 0.7 app with desktop and web features.
- Web entry is `web/index.html` with Trunk loading `../Cargo.toml` and `../assets/main.css`.
- Existing app injects `assets/main.css` for web in `src/main.rs`.
- Web dependencies already include `web-sys`, `wasm-bindgen`, `wasm-bindgen-futures`, `gloo-net`, and `gloo-timers`.

Likely build addition:

- Add an NPM/Vite/esbuild step under a new directory such as `web-ui/` or `assets/arwes/` to produce `assets/vendor/omegon-arwes.bundle.js`.
- Include that bundle from `web/index.html` with Trunk copy/script handling or a static `<script type="module">` once the output path is stable.
- For desktop, either include the same script in `custom_head` or keep the mockup web-only until the route is proven.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Arwes is alpha/not production-stable | Use it intentionally for MVP chrome; isolate behind custom elements so churn is contained. |
| React runtime inside Dioxus complicates lifecycle | Mount Arwes only in custom elements; Dioxus owns state and data. |
| Bundle size grows | Accept for decorative MVP; later tree-shake specific Arwes packages. |
| SSR/RSC/Strict Mode caveats | Not relevant to Dioxus CSR; do not enable React Strict Mode in the wrapper bundle. |
| Accessibility can suffer under decorative chrome | Keep semantic buttons/inputs in Dioxus where possible; Arwes wraps chrome, not core input semantics. |

## Mockup implementation target

The dummy-backed mockup should render one page:

- Arwes global background and framed shell.
- Top bar with agent identity, daemon state, workspace, model, context, transport.
- Main transcript with user/assistant/tool/lifecycle/approval events.
- Right context rail with runtime, workspace, memory/context, tools, workbench.
- Bottom composer with queue mode, command palette trigger, attach affordance, send/stop.
- Dummy interactions: send, stop, approve/deny, open command palette, open settings drawer.

The data provider should use contract-shaped fake data matching the planned Omegon Web surface snapshots and UI actions.
