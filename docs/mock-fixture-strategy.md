# Mock Fixture Strategy

## Purpose

Define how the Auspex scaffold should evolve from inline scenario constructors toward reusable host-session fixture packs.

## Current state

The scaffold already uses `MockHostSession::from_scenario(...)` to drive the UI.

That is acceptable for the first slices, but it is still coupling:
- fixture identity
- fixture content
- UI scenario switching

inside one implementation path.

## Next evolution

The next step should be to treat fixtures as named packs of host-session state.

Examples:
- `ready_session()`
- `booting_session()`
- `degraded_session()`
- `compatibility_failure_session()`
- `reconnecting_session()`

These can still be Rust constructors initially. They do not need to become external JSON or MessagePack fixtures yet.

## Why this matters

This separation will let Auspex evolve from:
- ad hoc scenario scaffolding

to:
- explicit reusable host-session fixture data

which is a better precursor to real host/runtime integration.

## Rule

Keep the first fixture layer code-native and simple. External fixture serialization can wait until it has real testing or relay-value.
