# auspex-data-model-v2 — Design

## Goal

Align Auspex with the Omegon 0.15.7 live-session surface by moving from a flat chat transcript model to a structured turn/block model.

## Decisions

1. Transcript state becomes turn-based.
2. Thinking blocks are distinct from response text and collapsed by default.
3. Tool calls become live cards with streamed partial output.
4. `message_abort` marks a partial assistant turn as aborted.
5. `context_updated` drives a persistent header gauge and a detailed session view.
6. Existing work/graph/session summary surfaces remain available.

## File scope

- `src/fixtures.rs` — new turn/block/view-model types; update mock session state
- `src/session_model.rs` — trait updates for turn-based transcript access
- `src/controller.rs` — controller compatibility and summary accessors
- `src/remote_session.rs` — event projection into turns/blocks
- `src/event_stream.rs` — add `context_updated` transport if needed in future state handling
- `src/app.rs` — render turn stream, tool cards, thinking blocks, header gauge
- `src/screens.rs` — Session screen token usage and richer session details
- `src/omegon_control.rs` — extend event model for `MessageAbort` and `ContextUpdated`

## Constraints

- Keep the implementation minimal enough to ship in one pass.
- Preserve current mock mode behavior while adding richer live-mode rendering.
- Do not remove existing summary screens.
- Keep the change compatible with the 0.15.7 Omegon wire format.

## Acceptance criteria

- Auspex renders a structured transcript from Omegon live events.
- Tool updates stream into the same visible card.
- Thinking is not merged into response text.
- Aborted messages remain visible but clearly marked.
- Context tokens are visible in the header and on the Session screen.
