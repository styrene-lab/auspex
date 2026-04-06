---
title: Omegon Unified Slash Command Contract
status: active
date: 2026-04-06
tags: [omegon, auspex, slash-commands, contract]
---

# Omegon Unified Slash Command Contract

This document answers the exact blocker raised by the Auspex agent:

> We are not blocked by Auspex architecture. We are blocked by contract certainty.

Correct. The current Omegon implementation now has a **single canonical remote slash execution path** for IPC, web, and tool-driven context commands. Auspex should bind to that path instead of inventing its own slash semantics.

## Short answer

The current in-flight Omegon branch state to bind against is represented by these recent commits in `../omegon`:

- `ff6e11f4 feat(omegon): route remote slash commands through canonical runtime executor`
- `b3d46c5d fix(web): return structured slash results to dashboard clients`
- `4e6186bb fix(context): wait for authoritative slash completion`

These establish the actual contract.

## Canonical execution path

All non-interactive slash callers should target the same runtime handler:

- `TuiCommand::RunSlashCommand { name, args, respond_to }`
  - Source: `../omegon/core/crates/omegon/src/tui/mod.rs:75-78`
- Main-loop handling routes this to:
  - `execute_remote_slash_command(...)`
  - Source: `../omegon/core/crates/omegon/src/main.rs:1427-1445`

This is the path already used by:

1. **IPC** `run_slash_command`
   - `../omegon/core/crates/omegon/src/ipc/connection.rs:301-335`
2. **WebSocket** `slash_command`
   - `../omegon/core/crates/omegon/src/web/ws.rs:168-200`
3. **Context tools** `context_compact` / `context_clear`
   - now internally re-routed through the same slash executor
   - `../omegon/core/crates/omegon/src/features/context.rs`

## Current machine contract

### Request shape

The canonical bridged request type is currently:

```rust
pub struct SlashCommandRequest {
    pub name: String,
    pub args: String,
}
```

Source:
- `../omegon/core/crates/omegon-traits/src/lib.rs:248-251`

### Response shape

The current structured response type is:

```rust
pub struct SlashCommandResponse {
    pub accepted: bool,
    pub output: Option<String>,
}
```

Source:
- `../omegon/core/crates/omegon-traits/src/lib.rs:253-258`

### Important caveat

OpenSpec says the long-term bridge should return a richer normalized envelope, but **the current implementation does not do that yet**.

Spec reference:
- `../omegon/openspec/baseline/harness/slash-commands.md`

Today, the real implementation is only:

- `accepted: bool`
- `output: Option<String>`

So Auspex should bind to the **actual current envelope**, not the aspirational richer spec.

## Current inbound payload shapes by surface

### IPC surface

Command name:
- `run_slash_command`

Payload JSON:

```json
{
  "name": "context",
  "args": "status"
}
```

Behavior:
- parsed as `SlashCommandRequest`
- forwarded to `TuiCommand::RunSlashCommand`
- waits for oneshot response
- returns serialized `SlashCommandResponse`

Source:
- `../omegon/core/crates/omegon/src/ipc/connection.rs:301-335`

### WebSocket surface

Inbound command message:

```json
{
  "type": "slash_command",
  "name": "context",
  "args": "status"
}
```

Behavior:
- forwarded as `WebCommand::SlashCommand { name, args, respond_to }`
- waits for completion
- emits back:

```json
{
  "type": "slash_command_result",
  "event_name": "slash.command.result",
  "name": "context",
  "args": "status",
  "accepted": true,
  "output": "..."
}
```

Sources:
- inbound command handling: `../omegon/core/crates/omegon/src/web/ws.rs:168-200`
- emitted result shape: `../omegon/core/crates/omegon/src/web/ws.rs:237-250`
- web command enum: `../omegon/core/crates/omegon/src/web/mod.rs:216-223`

## Canonical slash name mapping

The actual parser today is:
- `../omegon/core/crates/omegon/src/tui/mod.rs:265-294`

Canonical commands currently bridged are:

```rust
ModelList
SetModel(String)
SetThinking(ThinkingLevel)
ContextStatus
ContextCompact
ContextClear
SetContextClass(ContextClass)
NewSession
ListSessions
AuthStatus
AuthUnlock
AuthLogin(String)
AuthLogout(String)
```

Source:
- `../omegon/core/crates/omegon/src/tui/mod.rs:248-263`

## Exact slash spellings and args

These are the exact user-facing names Auspex should emit.

### Model

#### `/model list`
Maps to:
- `CanonicalSlashCommand::ModelList`

#### `/model <model-spec>`
Examples:
- `/model gpt-5.4`
- `/model anthropic/claude-sonnet-4.5`

Maps to:
- `CanonicalSlashCommand::SetModel(String)`

Current result:
- `accepted: true`
- `output`: multi-line text describing requested/resolved model and possible provider reroute

Handler output source:
- `../omegon/core/crates/omegon/src/main.rs:2388-2398`

### Thinking

#### `/think <level>`
Examples depend on current `ThinkingLevel::parse`, but contractually this is the parser path.

Maps to:
- `CanonicalSlashCommand::SetThinking(level)`

Current result example:
- `Thinking → <icon> <level>`

Source:
- `../omegon/core/crates/omegon/src/main.rs:2400-2407`

### Context

#### `/context status`
Maps to:
- `CanonicalSlashCommand::ContextStatus`

Result:
- `accepted: true`
- `output`: textual context snapshot

Example output shape:

```text
Context: 12345/200000 tokens (6%)
Class: Squad
Thinking: off
```

Source:
- `../omegon/core/crates/omegon/src/main.rs:2409-2428`

#### `/context compact`
Aliases:
- `/context compress`

Maps to:
- `CanonicalSlashCommand::ContextCompact`

Possible results:
- success:
  - `accepted: true`
  - `output: "Context compressed. Now using <n> tokens."`
- nothing eligible:
  - `accepted: true`
  - `output: "Nothing to compress yet — compaction only summarizes older turns after the decay window."`
- failure:
  - `accepted: false`
  - `output: "Compression failed: ..."`

Source:
- parser: `../omegon/core/crates/omegon/src/tui/mod.rs:272-280`
- execution: `../omegon/core/crates/omegon/src/main.rs:2430-2484`

#### `/context clear`
Maps to:
- `CanonicalSlashCommand::ContextClear`

Result:
- `accepted: true`
- `output: "Context cleared. Starting fresh conversation."`

Source:
- `../omegon/core/crates/omegon/src/main.rs:2486-2514`

#### `/context <class>`
Currently parsed as a context-class setter when the subcommand is not `status|compact|compress|clear` and `ContextClass::parse(sub)` succeeds.

Maps to:
- `CanonicalSlashCommand::SetContextClass(ContextClass)`

Result:
- `accepted: true`
- `output: "Context → <label>"`

Source:
- parse: `../omegon/core/crates/omegon/src/tui/mod.rs:272-280`
- result: `../omegon/core/crates/omegon/src/main.rs:2516-2528`

### Sessions

#### `/new`
Maps to:
- `CanonicalSlashCommand::NewSession`

Result:
- `accepted: true`
- `output: "Started a fresh session."`

Source:
- `../omegon/core/crates/omegon/src/main.rs:2530-2545`

#### `/sessions`
Maps to:
- `CanonicalSlashCommand::ListSessions`

Result:
- `accepted: true`
- `output`: rendered session list text

Source:
- `../omegon/core/crates/omegon/src/main.rs:2547-2550`

## Exact auth slash names / arguments / results

This is the part the Auspex agent explicitly asked for.

### `/auth`
### `/auth status`
Both map to:
- `CanonicalSlashCommand::AuthStatus`

Result:
- `accepted: true`
- `output`: formatted auth status summary across providers

Source:
- parser: `../omegon/core/crates/omegon/src/tui/mod.rs:284-287`
- execution: `../omegon/core/crates/omegon/src/main.rs:2551-2556`

### `/auth unlock`
Maps to:
- `CanonicalSlashCommand::AuthUnlock`

Current result:
- `accepted: true`
- `output: "🔒 Secrets store unlock not yet implemented"`

Important:
- This is effectively a placeholder, not a completed unlock flow.

Source:
- parser: `../omegon/core/crates/omegon/src/tui/mod.rs:284-287`
- execution: `../omegon/core/crates/omegon/src/main.rs:2558-2561`

### `/login <provider>`
Maps to:
- `CanonicalSlashCommand::AuthLogin(String)`

Recognized provider spellings in the current remote path:
- `anthropic`
- `claude`
- `openai-codex`
- `chatgpt`
- `codex`
- `openai` (special case: refused for remote/bridged login)

Behavior by provider:

#### `/login anthropic`
- `accepted: true`
- immediate output:
  - `Login started for anthropic. Complete any interactive prompts in the TUI.`
- completion happens asynchronously via system notifications in the TUI/web stream

#### `/login claude`
- same behavior as anthropic

#### `/login openai-codex`
#### `/login chatgpt`
#### `/login codex`
- `accepted: true`
- immediate output:
  - `Login started for <provider>. Complete any interactive prompts in the TUI.`
- completion also async via notifications

#### `/login openai`
- **refused in bridged mode**
- `accepted: false`
- output:
  - `OpenAI API login is interactive-only in the TUI. Use /login in the terminal session or set OPENAI_API_KEY.`

#### concurrent login already pending
- `accepted: false`
- output:
  - `Login is already waiting for interactive input in the TUI.`

#### unknown provider
- initial slash response still starts the async path for some cases, but the spawned task will eventually emit a failure notification:
  - `Unknown provider: X. Use: anthropic, openai, openai-codex`

Source:
- `../omegon/core/crates/omegon/src/main.rs:2562-2658`

### `/logout`
Default provider behavior:
- empty args default to `anthropic`

Parser behavior:

```rust
"logout" => Some(CanonicalSlashCommand::AuthLogout(
    if args.is_empty() { "anthropic" } else { args }.to_string(),
))
```

Source:
- `../omegon/core/crates/omegon/src/tui/mod.rs:289-292`

Examples:
- `/logout` → logs out `anthropic`
- `/logout anthropic`
- `/logout openai-codex`

Result currently:
- `accepted: true`
- output either:
  - `✓ Logged out from <provider>`
  - `❌ Logout failed: ...`

Important caveat:
- The current implementation always sets `accepted: true` even when logout fails, and encodes failure in prose. That is a contract weakness in the current build.

Source:
- `../omegon/core/crates/omegon/src/main.rs:2660-2668`

## What Auspex should implement now

Bind to the **actual current contract**, not the long-term richer OpenSpec shape.

### Recommended adapter contract inside Auspex

Represent outbound requests as:

```ts
interface OmegonSlashRequest {
  name: string;
  args: string;
}
```

Represent inbound results as:

```ts
interface OmegonSlashResponse {
  accepted: boolean;
  output?: string;
}
```

For WebSocket-specific result events:

```ts
interface OmegonSlashCommandResultEvent {
  type: 'slash_command_result';
  event_name: 'slash.command.result';
  name: string;
  args: string;
  accepted: boolean;
  output: string;
}
```

### Binding guidance

1. **Do not parse terminal prose to discover the command contract.**
   Bind to `name + args -> accepted + output`.
2. **Treat login as partially async.**
   The immediate slash response only confirms kickoff. Final success/failure can arrive later through streamed system notifications.
3. **Treat `/logout` failure text cautiously.**
   Current implementation may say `accepted: true` even when output contains failure prose.
4. **Prefer canonical slash names exactly as Omegon parses them now.**
5. **Do not invent alternate auth route names in Auspex.**
   Map your UI to these names.

## Recommended minimum command map for Auspex

If the goal is auth/settings/browser parity, the minimum safe set is:

- `/auth`
- `/auth status`
- `/login anthropic`
- `/login openai-codex`
- `/logout`
- `/logout anthropic`
- `/logout openai-codex`
- `/context status`
- `/context compact`
- `/context clear`
- `/model list`
- `/model <spec>`
- `/think <level>`

## Explicit answer to the agent’s 3 asks

### 1. Actual Omegon branch/diff/spec for the unified slash executor

Use these concrete anchors in `../omegon`:

- commits:
  - `ff6e11f4`
  - `b3d46c5d`
  - `4e6186bb`
- spec:
  - `../omegon/openspec/baseline/harness/slash-commands.md`
- implementation entry points:
  - `../omegon/core/crates/omegon/src/tui/mod.rs:248-294`
  - `../omegon/core/crates/omegon/src/main.rs:1427-1445`
  - `../omegon/core/crates/omegon/src/ipc/connection.rs:301-335`
  - `../omegon/core/crates/omegon/src/web/ws.rs:168-250`

### 2. Canonical inbound command payload shape

#### IPC

```json
{
  "name": "context",
  "args": "status"
}
```

#### WebSocket

```json
{
  "type": "slash_command",
  "name": "context",
  "args": "status"
}
```

#### Response today

```json
{
  "accepted": true,
  "output": "..."
}
```

#### WebSocket event form

```json
{
  "type": "slash_command_result",
  "event_name": "slash.command.result",
  "name": "context",
  "args": "status",
  "accepted": true,
  "output": "..."
}
```

### 3. Exact auth slash names / arguments / results

Use exactly:

- `/auth`
- `/auth status`
- `/auth unlock`
- `/login anthropic`
- `/login claude`
- `/login openai-codex`
- `/login chatgpt`
- `/login codex`
- `/login openai` → currently rejected in bridged mode
- `/logout`
- `/logout <provider>`

Current result semantics are documented above in detail.

## Bottom line

Auspex is no longer blocked on architecture. It now has the current Omegon contract.

The correct implementation move is:
- bind to the current `name + args -> accepted + output` remote slash bridge,
- use exact canonical slash spellings above,
- treat login as kickoff + async notifications,
- avoid inventing a richer envelope until Omegon actually ships one.
