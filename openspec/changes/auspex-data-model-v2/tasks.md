# auspex-data-model-v2 — Tasks

## 1. Transcript model
<!-- specs: frontend -->
- [ ] Introduce `Turn`, `TurnBlock`, and `ToolCard` view-model types in `src/fixtures.rs`
- [ ] Extend `RemoteHostSession` to project Omegon events into structured turns instead of a flat `Vec<ChatMessage>`
- [ ] Keep a compatibility layer for existing summary computations so work/session screens still function
- [ ] Add unit tests for turn ordering, block typing, and aborted message behavior

## 2. Event protocol coverage
<!-- specs: frontend -->
- [ ] Add `MessageAbort` and `ContextUpdated` variants to `OmegonEvent` in `src/omegon_control.rs`
- [ ] Deserialize the real Omegon 0.15.7 payload shapes for those new variants
- [ ] Update event projection code to handle `tool_update` and `message_abort` explicitly
- [ ] Add tests proving live tool updates and abort state are preserved in the model

## 3. Transcript rendering
<!-- specs: frontend -->
- [ ] Replace the transcript list in `src/app.rs` with a turn/block renderer
- [ ] Render thinking blocks as collapsed-by-default sections
- [ ] Render tool cards with args, partial output, result, and error state
- [ ] Render aborted messages with muted/struck-through styling

## 4. Context visibility
<!-- specs: frontend -->
- [ ] Add a context token field to the live session model and summary data
- [ ] Surface the token count in the header area as a compact gauge or meter
- [ ] Add a detailed context usage row to the Session screen in `src/screens.rs`
- [ ] Add tests for context token updates and header/session display data

## 5. Session and summary compatibility
<!-- specs: frontend -->
- [ ] Keep existing work/graph/session summary views working with the richer transcript model
- [ ] Update controller helpers and mock sessions to expose the new transcript structures
- [ ] Ensure the existing composer and submit/cancel flows still behave correctly
- [ ] Add regression tests for mock mode and live mode summary rendering
