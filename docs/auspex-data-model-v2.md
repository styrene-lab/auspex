---
id: auspex-data-model-v2
title: "Auspex frontend data model v2 — live session fidelity"
status: implemented
tags: []
open_questions: []
dependencies: []
related: []
---

# Auspex frontend data model v2 — live session fidelity

## Overview

Align the Auspex frontend data model with the full surface Omegon 0.15.7 exposes, prioritizing the live session experience (tool cards, thinking/response split, context tokens, message abort) over the secondary work/graph screens.

## Research

### Omegon 0.15.7 WS event protocol (from source)

Events are JSON with a `type` field, HTML-escaped for web safety.

**Turn lifecycle:**
- `turn_start { turn: u32 }`
- `turn_end { turn: u32 }`

**Message streaming:**
- `message_start { role: "assistant"|"user" }`
- `message_chunk { text: string }` — response text, HTML-escaped
- `thinking_chunk { text: string }` — extended thinking, HTML-escaped
- `message_end` — completes current message
- `message_abort` — aborts current message (new in 0.15.7)

**Tool lifecycle:**
- `tool_start { id: string, name: string, args: Value }` — tool invocation begins
- `tool_update { id: string, partial: string }` — streaming partial result, HTML-escaped
- `tool_end { id: string, result: string, is_error: bool, block_count: usize }` — tool completed

**Session events:**
- `agent_end` — agent turn finished
- `phase_changed { phase: string }` — lifecycle phase (e.g. "Idle")
- `context_updated { tokens: Value }` — context window token usage
- `harness_status_changed { status: HarnessStatusSnapshot }` — full harness state
- `state_snapshot { data: StateSnapshot }` — full state resync
- `system_notification { message: string }` — informational notice
- `session_reset` — session cleared

**Decomposition:**
- `decomposition_started { children: Vec<string> }`
- `decomposition_child_completed { label: string, success: bool }`
- `decomposition_completed { merged: bool }`

**Inbound commands (client → server):**
- `user_prompt { text: string }`
- `slash_command { name: string, args: string }`
- `cancel`
- `request_snapshot`

### Current Auspex transcript model (gap baseline)

Current Auspex transcript model is `Vec<ChatMessage>` where:
```rust
struct ChatMessage { role: MessageRole, text: String }
enum MessageRole { User, Assistant, System }
```

This is flat — no concept of turns, tool calls, thinking blocks, or streaming state. All tool events collapse to activity label strings. Thinking chunks merge into message text.

The `RemoteHostSession` tracks:
- `pending_role: Option<MessageRole>` — current message being assembled
- `pending_text: String` — accumulated text for current message
- `run_active: bool` — whether agent turn is in progress

No per-tool-call state is tracked. No thinking vs response distinction exists. No context token tracking.

The `OmegonEvent` enum already models all event types except `MessageAbort` and `ContextUpdated`.

### Cowork Desktop paradigm — block stream model

In the Cowork Desktop paradigm, a single agent turn renders as a vertical stream of blocks:

1. **Thinking block** (collapsible, muted) — shows extended thinking if present
2. **Response text** — assistant's natural language
3. **Tool call card** — for each tool invocation:
   - Tool name as header
   - Args shown (possibly truncated)
   - Streaming output while running
   - Final result on completion
   - Error state distinct from success
4. **More response text** — interleaved with tool calls
5. **Turn summary** — tool count, token usage

Key insight: this is a **block stream**, not a message list. Within a single turn, text chunks, thinking chunks, and tool calls interleave. The current flat `Vec<ChatMessage>` cannot represent this.

Proposed replacement: a `Turn` model containing ordered `TurnBlock` entries:
- `TurnBlock::Thinking(String)` — collapsed by default
- `TurnBlock::Text(String)` — response text
- `TurnBlock::ToolCall { id, name, args, status, output }` — individual tool
- `TurnBlock::System(String)` — system notices within a turn

## Decisions

### Replace flat ChatMessage vec with structured Turn model

**Status:** accepted

**Rationale:** The flat Vec<ChatMessage> cannot represent interleaved tool calls, thinking blocks, and response text within a single agent turn. A Turn model with ordered TurnBlock entries is the minimum viable structure for the Cowork paradigm. User prompts remain simple text entries between turns.

### Tool calls rendered as collapsible cards with name, args summary, streaming output, and final result

**Status:** accepted

**Rationale:** This is the core of the Cowork paradigm — seeing what the agent is doing. tool_update partial results should stream live into the card (not just show final result), because watching the agent work is a primary value prop.

### Thinking blocks visually distinct — collapsed by default, expandable

**Status:** accepted

**Rationale:** Extended thinking is useful for understanding the agent's reasoning but noisy for normal operation. Collapsed-by-default with expand toggle matches both Cowork Desktop and the VS Code Copilot pattern.

### Context token gauge in header area (persistent), detailed breakdown in session screen

**Status:** accepted

**Rationale:** Context window fullness is critical operational awareness — the user needs to see it approaching limits without navigating to a separate screen. A compact gauge in the header (like a battery indicator) with detailed numbers on the session screen.

### message_abort marks pending message as aborted (strikethrough/muted) rather than discarding

**Status:** accepted

**Rationale:** Discarding silently is confusing — the user typed cancel and needs visual confirmation it worked. Showing the partial text with an "aborted" indicator is clearer.
