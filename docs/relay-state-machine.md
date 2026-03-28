# Relay State Machine

## Purpose

Define the host-side lifecycle and transition model for the desktop Auspex relay.

This is the implementation-near state machine that ties together:
- subsystem supervision
- Omegon compatibility
- host session cache
- phone relay availability

## State domains

The desktop host needs at least two linked state machines:

### 1. Local host engine state
Tracks the host's relationship to bundled Omegon and Styrene.

### 2. Remote relay session state
Tracks whether a phone client can attach and what quality of session it receives.

## Local host engine states

### `booting`
Auspex shell is starting.

### `styrene_starting`
Styrene runtime/daemon is starting or being attached.

### `styrene_failed`
Styrene runtime is unavailable or unhealthy.

### `omegon_starting`
Omegon engine is starting or being attached.

### `compatibility_checking`
Omegon startup metadata has been read and is being validated.

### `omegon_incompatible`
Omegon is reachable but fails version/schema compatibility.

### `session_building`
Desktop host cache and local session structures are being established.

### `ready`
Desktop-local session is operational.

### `degraded`
The session exists but one or more subsystems are impaired.

### `failed`
The host cannot enter a usable session.

## Remote relay session states

### `relay_unavailable`
Phone clients cannot attach yet.

### `relay_starting`
Host is preparing the remote relay surface.

### `relay_ready`
Phone clients may attach.

### `relay_connected`
At least one phone client is connected.

### `relay_degraded`
Remote relay exists but host/session quality is impaired.

### `relay_disconnected`
A previously attached phone client disconnected.

## Transition sketch

### Boot path
`booting`
-> `styrene_starting`
-> `omegon_starting`
-> `compatibility_checking`
-> `session_building`
-> `ready`

If relay is enabled:
`ready` -> `relay_starting` -> `relay_ready`

### Failure examples
- Styrene failure before host session: `styrene_starting` -> `styrene_failed` -> `failed` or degraded local-only mode
- Omegon compatibility mismatch: `compatibility_checking` -> `omegon_incompatible` -> `failed`
- Omegon runtime failure after session exists: `ready` -> `degraded`
- Relay failure while local session still works: `relay_connected` -> `relay_degraded` or `relay_disconnected`

## Event consequences

### Host state changes should emit
- local status updates for desktop UI
- compatibility/degradation updates into the phone-facing projection when relay is active

### Relay state changes should emit
- connection status changes to the phone client
- session summary refreshes when needed

## Phone-visible implications

### If host is `ready` and relay is `relay_ready`
Phone can attach normally.

### If host is `degraded`
Phone should still receive projected state, but with degraded connection/session indicators.

### If host is `failed`
Phone should not attempt normal session operation.

### If relay is `relay_disconnected`
Phone should show a host-link disconnect state, not an Omegon engine failure.

## Recovery rules

### Rule 1
Do not destroy the session cache on every transient reconnect unless correctness demands it.

### Rule 2
Prefer demoting to `degraded` over hard-failing when the host can still explain state and recover.

### Rule 3
Compatibility mismatch is a hard stop, not a degraded mode.

## Guiding rule

The relay state machine should make it obvious which layer is failing:
- Styrene transport
- Omegon engine
- compatibility gate
- host session cache
- remote relay link
