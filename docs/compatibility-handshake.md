# Compatibility Handshake

## Purpose

Define how Auspex verifies that a running Omegon instance is compatible before it tries to operate against the control plane.

This is the runtime half of the release dependency policy.

## Inputs

Auspex should compare:
- its own declared compatibility manifest
- Omegon startup/discovery metadata
- Omegon HTTP snapshot metadata

## Required manifest inputs on the Auspex side

Current Auspex declaration:

```toml
[package.metadata.omegon]
minimum_version = "0.15.4-rc.16"
maximum_tested_version = "0.15.4-rc.16"
control_plane_schema = 1
```

General shape:

```toml
[package.metadata.omegon]
minimum_version = "0.16.0"
maximum_tested_version = "0.16.x"
control_plane_schema = 1
```

## Required runtime inputs on the Omegon side

Auspex must be able to read at least:
- `omegonVersion`
- `schemaVersion`

Preferably from both:
- startup/discovery output
- `/api/state`

## Handshake sequence

### 1. Launch or attach
Auspex launches Omegon or attaches to an existing control-plane endpoint.

### 2. Read startup/discovery metadata
Auspex reads:
- process binding information
- auth token
- Omegon version
- control-plane schema version

### 3. Validate hard compatibility
Auspex verifies:
- Omegon version is within the supported release line
- control-plane schema matches exactly

### 4. Optionally confirm through `/api/state`
After connect, Auspex may verify that `/api/state` reports the same:
- `omegonVersion`
- `schemaVersion`

### 5. Proceed or fail hard
If compatibility passes, Auspex continues.
If compatibility fails, Auspex must not silently continue in normal mode.

## Outcomes

### Compatible
Auspex continues to:
- fetch snapshot state
- connect WebSocket
- render UI normally

### Incompatible schema
Auspex must stop and show a compatibility error.

Example:
- `Auspex requires control-plane schema 1, but Omegon reported schema 2.`

### Unsupported release line
Auspex must stop and show a release compatibility error.

Example:
- `Auspex supports Omegon 0.16.x. Connected instance is 0.17.0.`

### Missing version identity
Auspex should treat a missing version/schema identity as incompatible.

That is stricter than permissive fallback, and that is correct while the contract is still young.

## UI requirements for compatibility failure

The compatibility failure surface should:
- be visible before normal app operation starts
- explain what was expected
- explain what was received
- tell the operator what to update

### Must not
- fail silently
- half-render the app and hope for the best
- bury the mismatch in logs only

## Suggested compatibility screen

Show:
- Auspex version
- supported Omegon release line
- supported control-plane schema
- detected Omegon version
- detected schema version
- recommended next action

## Special case: development mode

A future development override may exist, but it should be explicit and noisy.

Example:
- `--allow-unsupported-omegon`

That must not become the normal compatibility model.

## Guiding rule

Compatibility negotiation should fail early, visibly, and deterministically.
