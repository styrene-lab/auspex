# Supervision and Startup States

## Purpose

Define the high-level lifecycle and supervision states Auspex should expose while acting as the host for bundled Omegon and Styrene subsystems.

## State layers

Auspex should track at least:
- app shell state
- Styrene runtime state
- Omegon engine state
- host session state
- remote relay state

## Suggested startup flow states

### `booting`
Auspex shell is starting.

### `starting_styrene`
Styrene runtime is starting or being attached.

### `starting_omegon`
Omegon engine is starting or being attached.

### `validating_compatibility`
Auspex is checking Omegon version/schema compatibility.

### `session_initializing`
Desktop host session cache and relay state are being established.

### `ready`
Local session is operational.

### `degraded`
One or more subsystems are impaired but the shell can still report and potentially recover.

### `failed`
The host cannot continue normal operation.

## Recovery principle

Auspex should try to recover subsystem failures where sensible, but must always surface which layer failed:
- Styrene failure
- Omegon failure
- compatibility mismatch
- host relay failure

Do not collapse all of these into a single vague startup error.
