# auspex-data-model-v2 — Tasks

## 1. Transcript model
<!-- specs: frontend -->
- [x] Introduce `Turn`, `TurnBlock`, and `ToolCard` view-model types in `src/fixtures.rs`
- [x] Extend `RemoteHostSession` to project Omegon events into structured turns instead of a flat `Vec<ChatMessage>`
- [x] Keep a compatibility layer for existing summary computations so work/session screens still function
- [x] Add unit tests for turn ordering, block typing, and aborted message behavior

## 2. Event protocol coverage
<!-- specs: frontend -->
- [x] Add `MessageAbort` and `ContextUpdated` variants to `OmegonEvent` in `src/omegon_control.rs`
- [x] Deserialize the real Omegon 0.15.7 payload shapes for those new variants
- [x] Update event projection code to handle `tool_update` and `message_abort` explicitly
- [x] Add tests proving live tool updates and abort state are preserved in the model

## 3. Transcript rendering
<!-- specs: frontend -->
- [x] Replace the transcript list in `src/app.rs` with a turn/block renderer
- [x] Render thinking blocks as collapsed-by-default sections
- [x] Render tool cards with args, partial output, result, and error state
- [x] Render aborted messages with muted/struck-through styling

## 4. Context visibility
<!-- specs: frontend -->
- [x] Add a context token field to the live session model and summary data
- [x] Surface the token count in the header area as a compact gauge or meter
- [x] Add a detailed context usage row to the Session screen in `src/screens.rs`
- [x] Add tests for context token updates and header/session display data

## 5. Session and summary compatibility
<!-- specs: frontend -->
- [x] Keep existing work/graph/session summary views working with the richer transcript model
- [x] Update controller helpers and mock sessions to expose the new transcript structures
- [x] Ensure the existing composer and submit/cancel flows still behave correctly
- [x] Add regression tests for mock mode and live mode summary rendering

## Reconciliation notes

- Structured transcript rendering is live in `src/app.rs` via `render_transcript`.
- `src/remote_session.rs` now projects `ThinkingChunk`, `MessageAbort`, `ToolUpdate`, and `ContextUpdated` into the turn/block model and session/context state.
- `src/screens.rs` now surfaces context usage in the Session screen in addition to the header summary.
- Session chrome semantics were further tightened after the original plan: activity-strip state and top-level bootstrap/startup/reconnect/failure surfaces are now typed instead of inferred from prose.
