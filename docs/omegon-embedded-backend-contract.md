# Omegon Embedded Backend Contract for Auspex

## Purpose

Define the exact backend contract Auspex needs from Omegon for the first full patch release that includes a working embedded local backend.

This is an implementation-facing contract for the Omegon agent workstream.

It is intentionally stricter than a long-term public API design:
- down-selection later is cheap
- reverse-engineering a half-specified runtime boundary is expensive

## Status

This is the **required target contract** from Auspex's side.

It is not a claim that Omegon already implements all of this.

## Product stance

For Auspex MVP and the next full patch release:
- local embedded Omegon is the **default backend mode**
- remote attach is a **secondary explicit mode**
- mock fallback is **not** the product path for local startup failure

That means Omegon must provide a machine-oriented launch contract that Auspex can supervise directly.

---

## 1. Launch contract

### Required invocation shape

Omegon must provide a dedicated machine-start mode for Auspex.

Accepted examples:
- `omegon serve`
- `omegon control-plane`
- `omegon interactive --control-plane-only`
- `omegon interactive --headless --control-plane`

The exact CLI spelling is Omegon-owned.

The contract requirement is:
- Auspex can launch **one deterministic command**
- that command starts the local control plane without requiring TUI interaction
- that command is stable enough to pin to a release line

### Must not require
- slash commands like `/dash`
- simulated keypresses
- scraping terminal logs for URLs/tokens
- browser-opening side effects as the only startup behavior
- a visible TUI as part of the happy path for embedded backend mode

### Required process behavior

The machine-start process must:
- stay alive until explicitly terminated or until Omegon exits on error
- expose machine-readable startup/discovery metadata
- expose the HTTP + WebSocket control plane
- return a non-zero exit code on fatal startup failure

### Required stdout/stderr behavior

The machine-start mode must be safe to launch from a GUI host.

That means:
- no ANSI/TUI escape sequence spam as the primary output contract
- human-readable logs are fine on stderr/stdout
- but Auspex must not need to parse them for startup correctness

---

## 2. Startup/discovery contract

Auspex needs a machine-readable startup identity surface.

### Required delivery options

At least one of these must exist:
1. startup JSON written to stdout on successful launch
2. a startup file path explicitly provided by arg/env
3. an HTTP startup endpoint available immediately after bind

Preferred model for Auspex:
- launch Omegon machine-start mode
- wait for `GET /api/startup`
- read startup metadata there

### Required startup payload

```json
{
  "omegonVersion": "0.15.4-rc.19",
  "schemaVersion": 1,
  "pid": 12345,
  "cwd": "/path/to/repo",
  "addr": "127.0.0.1:7842",
  "httpBase": "http://127.0.0.1:7842",
  "stateUrl": "http://127.0.0.1:7842/api/state",
  "wsUrl": "ws://127.0.0.1:7842/ws?token=...",
  "token": "...",
  "authMode": "signed-attach",
  "authSource": "keyring"
}
```

### Required fields
- `omegonVersion: string`
- `schemaVersion: integer`
- `pid: integer`
- `cwd: string`
- `addr: string`
- `httpBase: string`
- `stateUrl: string`
- `wsUrl: string`
- `token: string`
- `authMode: string`
- `authSource: string`

### Semantics
- `omegonVersion` must be the released application version, not a branch name
- `schemaVersion` is the control-plane compatibility contract version
- `stateUrl` and `wsUrl` must be directly usable by Auspex
- `token` must be valid for the corresponding WebSocket attach path

### Required startup guarantees
- if startup succeeds, payload must be internally consistent
- if startup payload is exposed, the HTTP server must already be bound or become reachable immediately after
- missing `omegonVersion` or `schemaVersion` is a compatibility failure from Auspex's perspective

---

## 3. HTTP contract

HTTP is read-only in v1.

### Required routes
- `GET /api/startup`
- `GET /api/state`
- `GET /api/graph`

### Recommended slice routes
- `GET /api/design-tree`
- `GET /api/openspec`
- `GET /api/cleave`
- `GET /api/harness`
- `GET /api/health`

Slice routes are optional for the immediate embedded-backend milestone.

### HTTP requirements
- JSON responses only
- stable field naming
- non-2xx on failure
- no mutation routes required for Auspex v1

---

## 4. `/api/state` contract

Auspex needs one normalized snapshot it can trust at attach time.

### Required top-level shape

```json
{
  "schemaVersion": 1,
  "omegonVersion": "0.15.4-rc.19",
  "session": {},
  "designTree": {},
  "openspec": {},
  "cleave": {},
  "harness": {},
  "health": {}
}
```

### Required top-level fields
- `schemaVersion`
- `omegonVersion`
- `session`
- `designTree`
- `openspec`
- `cleave`
- `harness`
- `health`

### Session section

Required fields:
- `cwd`
- `pid`
- `startedAt`
- `turns`
- `toolCalls`
- `compactions`
- `gitBranch`
- `gitDetached`

Optional but useful:
- `sessionId`
- `activeRun`
- `lastPromptAt`

### Design tree section

Required fields:
- `focused`
- `implementing`
- `actionable`
- `nodes`
- `counts`

#### Focused node shape
- `id`
- `title`
- `status`
- `openQuestions`
- `decisions`
- `children`

#### Node shape
- `id`
- `title`
- `status`

### OpenSpec section

Required fields:
- `totalTasks`
- `doneTasks`

Optional but useful:
- `activeChanges`
- `changes`

### Cleave section

Required fields:
- `active`
- `totalChildren`
- `completed`
- `failed`

Optional but useful:
- `children`

### Harness section

Auspex wants the latest coherent harness snapshot in the initial HTTP state.

Required fields:
- `gitBranch`
- `gitDetached`
- `thinkingLevel`
- `capabilityTier`
- `providers`
- `memoryAvailable`
- `cleaveAvailable`
- `memoryWarning`
- `activeDelegates`

Provider entries should include:
- `name`
- `authenticated`
- `authMethod`
- `model`

### Health section

Required fields:
- `status`
- `lastUpdatedAt`

Recommended fields:
- `controlPlaneReady`
- `wsReady`
- `agentReady`

---

## 5. `/api/graph` contract

Auspex does not need a perfect graph system for the embedded-backend milestone, but it does need a public shape that is not accidental.

### Required graph response shape

```json
{
  "nodes": [],
  "edges": []
}
```

### Required node fields
- `id`
- `title`
- `group`
- `status`

### Required edge fields
- `source`
- `target`
- `type`

### Required documented semantics
- allowed `group` values
- allowed edge `type` values
- whether omitted nodes are allowed

---

## 6. WebSocket contract

WebSocket is the write/live-event channel.

### Route
- `WS /ws?token=...`

### Authentication

Required:
- tokenized attach support
- invalid token must fail clearly
- reconnect with fresh token must be possible

### Required inbound commands from Auspex

At minimum:

#### User prompt
```json
{ "type": "user_prompt", "text": "..." }
```

#### Cancel
```json
{ "type": "cancel" }
```

### Recommended inbound commands
- slash command execution
- explicit snapshot refresh
- ping/keepalive

### Required outbound events from Omegon

At minimum:
- `state_snapshot`
- `message_start`
- `message_chunk`
- `thinking_chunk`
- `message_end`
- `system_notification`
- `harness_status_changed`
- `session_reset`
- `turn_start`
- `turn_end`
- `tool_start`
- `tool_update`
- `tool_end`
- `agent_end`
- `phase_changed`
- `decomposition_started`
- `decomposition_child_completed`
- `decomposition_completed`

### Required event semantics
- event names must be stable
- field names must be stable
- order must be coherent enough for transcript reconstruction
- `state_snapshot` must be usable as a full resync primitive

### Required reconnect behavior
- reconnecting clients must be able to recover by fetching `/api/state`
- optionally receive an immediate `state_snapshot`
- protocol must not require Auspex to preserve opaque in-memory session data just to reconnect safely

---

## 7. Supervision and readiness semantics

Auspex is the supervisor. Omegon is the supervised backend.

### Required readiness distinction

Omegon should distinguish:
- process started
- control plane bound
- startup payload available
- agent/session ready

Auspex only needs enough information to avoid false positives.

### Minimal acceptable behavior
- if `/api/startup` returns success, `/api/state` must be imminently available
- if Omegon cannot initialize the control plane, it must fail visibly rather than hang forever

### Timeout reality

Auspex may still impose a startup timeout.

That is not permission to degrade to mock mode.
It is supervision hygiene.

If the timeout is hit, Auspex will surface a startup failure screen.

---

## 8. Compatibility contract

### Required runtime identity

Auspex must be able to read:
- `omegonVersion`
- `schemaVersion`

from startup and preferably also state.

### Required compatibility behavior
- if `schemaVersion` mismatches: Auspex fails hard
- if `omegonVersion` is outside declared support bounds: Auspex fails hard
- if version identity is missing: Auspex treats that as incompatible

### MVP release coupling rule

For the next full patch release, target a single release line.

Current intended line while implementation is underway:
- Omegon `0.15.4-rc.19`
- control-plane schema `1`

The final patch release may update this exact version before ship, but the contract must support explicit pinning.

---

## 9. Failure contract

Auspex needs backend failures to be explainable.

### Required startup failure modes

Omegon machine-start mode must make these distinguishable:
- binary launch failure
- bind failure
- auth/token initialization failure
- control-plane initialization failure
- incompatible schema/version identity

### Required behavior
- non-zero exit on fatal unrecoverable startup failure
- human-readable error output is welcome
- but machine-readable startup correctness must not depend on parsing that output

---

## 10. Non-goals for this milestone

The implementing agent does **not** need to solve all future backend concerns now.

Out of scope unless they fall out naturally:
- write-capable HTTP routes
- full remote phone relay semantics
- final long-term public API naming if a temporary compatibility shim is needed
- in-process embedding
- multi-tenant hosting

---

## 11. Acceptance criteria from Auspex's perspective

The contract is sufficient when all of the following are true:

1. Auspex can launch a deterministic Omegon machine-start command.
2. No TUI interaction is required.
3. No raw terminal escape contract is involved.
4. `/api/startup` returns machine-readable startup metadata.
5. `/api/state` returns a normalized snapshot with `omegonVersion` and `schemaVersion`.
6. `/ws` accepts tokenized attach and streams stable events.
7. Auspex can submit prompts and cancel over WebSocket.
8. Auspex can fail clearly on startup or compatibility problems.
9. Auspex can declare and verify a pinned Omegon release line.

---

## 12. Handoff request to the Omegon agent

Implement more than the minimum if that is cheaper inside Omegon, then report back with:
- the actual launch command(s)
- actual startup payload shape
- actual `/api/state` shape
- actual `/api/graph` shape
- actual WebSocket command/event contract
- any deviations from this target contract
- which fields are stable now vs provisional

Down-selection on the Auspex side will be much cheaper than underspecifying the backend seam now.
