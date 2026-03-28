# Host Event Projection Rules

## Purpose

Define which host-side state transitions and backend events should produce phone-facing projected updates.

This complements the relay state machine by specifying what the phone should be told when the host changes state.

## Projection triggers

### 1. Compatibility result changes
When the host determines or loses compatibility state, project:
- `compatibility_status`
- `session_summary` refresh if needed

### 2. Connection state changes
When host <-> Omegon or phone <-> host state changes, project:
- `connection_status`
- `session_summary` refresh if user-visible fields changed

### 3. Transcript changes
When transcript-relevant events arrive from Omegon, project:
- `transcript_chunk`
- `run_status` if turn/run lifecycle changed

### 4. Activity changes
When tool/run/parallel-work state materially changes, project:
- `activity_update`
- `run_status` when appropriate

### 5. Work-context changes
When focused node or compact work state changes materially, project:
- `session_state` or work summary refresh

### 6. Degradation changes
When the host enters or leaves degraded mode, project:
- `system_notice`
- `connection_status`
- `session_summary` refresh

## Suppression rule

Do not forward every low-level event if it does not change the phone-facing projection materially.

The host should project state changes, not event noise.

## Guiding rule

The phone should receive updates when meaning changes, not merely when backend chatter occurs.
