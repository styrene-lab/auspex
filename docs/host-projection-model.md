# Host Projection Model

## Purpose

Define how the desktop Auspex host reduces Omegon's `ControlPlaneStateV1` into the phone-facing remote session model.

This is the missing layer between:
- Omegon control-plane contract
- desktop Styrene relay session
- phone UI surfaces

## Core decision

The desktop host should project a **reduced, session-oriented state model** for the phone rather than forwarding the entire Omegon snapshot verbatim.

That reduction should be:
- deterministic
- documented
- stable enough for the first remote phone client

## Projection layers

### Layer 1 — Omegon source state
The host reads:
- `ControlPlaneStateV1`
- `/api/graph` when needed
- WebSocket events

### Layer 2 — Desktop host cache
The host maintains a normalized local cache representing the active session.

### Layer 3 — Remote phone session projection
The host projects a phone-facing remote state optimized for:
- Simple mode first
- low-bandwidth / reconnect-friendly operation
- less backend noise

## Proposed phone-facing state shape

```rust
pub struct RemotePhoneSessionState {
    pub session_summary: RemoteSessionSummary,
    pub transcript: RemoteTranscriptState,
    pub activity: RemoteActivityState,
    pub work: RemoteWorkSummary,
    pub compatibility: RemoteCompatibilityState,
    pub connection: RemoteConnectionState,
}
```

This is not the final wire format, but it is the right semantic shape.

## Section projections

### 1. `session_summary`
Derived from:
- `session`
- selected compact `harness` fields
- host metadata

Include:
- host label
- repo/session label
- connected/degraded state
- current branch
- compact provider/model summary if useful
- whether Power mode is available

### 2. `transcript`
Derived from:
- relayed WebSocket transcript events
- cached recent conversation state on the desktop host

Phone does not need raw `AgentEvent` parity. It needs coherent transcript state.

### 3. `activity`
Derived from:
- tool lifecycle events
- run lifecycle events
- `cleave` summary
- selected `harness` degradation/routing info

This should collapse noisy low-level behavior into plain-language current activity.

### 4. `work`
Derived from:
- `designTree.focused`
- `designTree.implementing`
- `openspec`
- `cleave`

Phone Simple mode should receive only compact work state.

Suggested compact fields:
- focused item summary
- active implementation count
- compact OpenSpec progress summary
- parallel task summary when active

### 5. `compatibility`
Derived from:
- desktop host compatibility handshake result

Phone should not initially negotiate Omegon compatibility directly. It should consume the host's compatibility status.

### 6. `connection`
Derived from two links:
- desktop host <-> Omegon
- phone <-> desktop host

The phone must be able to distinguish:
- host disconnected from Omegon
- phone disconnected from host
- host reconnecting to Omegon

## What should be omitted from the first phone projection

Do not forward by default:
- full `designTree.nodes`
- full `harness` inventory
- raw tool event stream
- raw thinking stream
- full graph payload unless requested
- every system status field from the desktop host

That data can be added later for Power mode or on-demand fetches.

## Projection rules for Simple mode first

### Always include
- transcript
- connection state
- run status
- current activity
- focused work summary
- cancel capability

### Include only when actionable
- degradation detail
- parallel-task detail
- provider/routing fallback detail
- deeper work progress detail

### Exclude by default
- graph payload
- deep session diagnostics
- raw lifecycle counters
- low-level backend noise

## Power mode extension path

The remote phone session model should be extensible.

That means Power mode can later request or receive:
- graph projections
- richer work state
- more session/system detail
- selected tool details

But the first remote shape should optimize for Simple mode correctness and clarity.

## Synchronization rule

The phone should refresh from projected host state, not re-derive its own view from raw event fragments.

That means:
- host owns reduction
- phone owns presentation

## Guiding rule

The desktop host should absorb backend complexity so the phone can consume a stable remote session abstraction.
