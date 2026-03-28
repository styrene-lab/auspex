# Error and Empty State Specification

## Purpose

Define the operator-facing states Auspex must handle before the full app is available.

This complements the compatibility handshake and screen bindings by specifying what the UI should show when the backend is unavailable, incompatible, empty, or still starting.

## State categories

### 1. Launching
Auspex has started a local Omegon process and is waiting for control-plane discovery.

#### UI requirements
Show:
- that Omegon is starting
- the current repository/session target if known
- a cancellable waiting state if supported

Must not:
- show a blank window with no explanation

### 2. Connecting
Auspex knows the target endpoint and is waiting for HTTP/WS readiness.

#### UI requirements
Show:
- that connection is in progress
- whether HTTP snapshot or WebSocket is still pending
- a retry or cancel path when practical

### 3. Compatibility failure
The connected Omegon instance does not satisfy the declared manifest.

#### UI requirements
Show:
- supported Omegon version range
- supported schema version
- detected Omegon version
- detected schema version
- recommended next action

This must block normal app operation.

### 4. Backend unavailable
Auspex cannot launch Omegon or cannot reach an expected control-plane endpoint.

#### UI requirements
Show:
- what failed (launch vs attach vs connect)
- enough detail to distinguish local startup failure from network/connectivity failure
- a retry path

### 5. Empty session
A compatible Omegon session exists, but there is little or no work state yet.

Examples:
- no focused node
- no active OpenSpec changes
- no cleave activity
- no prior transcript in a fresh session

#### UI requirements
Simple mode should still feel intentional, not broken.

Show:
- transcript area ready for first prompt
- compact status that session is connected
- if useful, a short first-action suggestion

### 6. Reconnecting
Auspex had a working session and temporarily lost WebSocket or HTTP connectivity.

#### UI requirements
Show:
- reconnecting state
- whether cached state is being shown
- whether user actions are temporarily disabled

### 7. Partial degradation
Some control-plane data is unavailable but the session is still broadly usable.

Examples:
- graph route unavailable
- harness status temporarily stale
- WebSocket reconnecting while last snapshot remains valid

#### UI requirements
Do not collapse into a fatal error if the app can still function.

Show:
- what is degraded
- what still works
- whether retry is automatic

## Mode-specific guidance

### Simple mode
- plain-language messaging
- minimal diagnostics in the default view
- optional details affordance

### Power mode
- more explicit diagnostics allowed
- may show route/protocol distinctions directly

## Guiding rule

A missing or degraded backend state should always render as a deliberate UI state, never as absence of UI.
