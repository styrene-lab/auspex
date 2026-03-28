# Embedded Runtime Model

## Purpose

Define how Auspex should bundle and manage Omegon and Styrene as first-party embedded subsystems.

This document answers the architectural question:
- should Auspex merely wrap external tools?
- or should it act as the sovereign host application for bundled engine and transport components?

## Core decision

Auspex should be the first-party host application that bundles and manages:
- the Omegon binary
- the Styrene runtime/daemon

But it should do so without collapsing subsystem boundaries.

That means:
- Auspex is the product shell
- Omegon remains the reasoning/action engine
- Styrene remains the identity/communications substrate

## Preferred runtime shape

### Near-term model

```text
Auspex.app
├── UI shell
├── session/orchestration layer
├── bundled omegon binary
├── bundled styrene runtime/daemon
└── supervision + compatibility + relay glue
```

This is the right interpretation of “Auspex should be the thing” without turning the system into a monolith.

## What Auspex owns

Auspex should own:
- packaging
- first-party operator experience
- subsystem startup/shutdown ordering
- compatibility verification
- health supervision
- session hosting
- remote relay hosting for phone clients

## What Auspex should not absorb

Auspex should not absorb:
- Omegon’s agent/runtime internals
- Styrene’s identity and comms semantics
- LLM backend semantics

Those subsystems should remain self-managing within their own domains.

## Process model options

### Option A — separate managed subprocesses
Auspex launches:
- Omegon as a managed subprocess
- Styrene as a managed subprocess/daemon

#### Assessment
This is the best first implementation path.

#### Why
- preserves fault isolation
- reuses existing binaries
- avoids premature library coupling
- makes supervision explicit

### Option B — mixed embedding
Auspex links some Styrene functionality directly while still managing Omegon as a subprocess.

#### Assessment
Possible later, but not the first step.

### Option C — fully in-process embedding
Auspex links Omegon and Styrene as internal libraries/subsystems in one runtime.

#### Assessment
Wrong first move. Too much coupling, too little fault isolation, and not justified yet.

## Recommended startup order

### 1. Auspex shell starts
- load settings
- determine local/remote session intent
- initialize supervision state

### 2. Styrene runtime becomes available
- start or attach to bundled Styrene runtime/daemon
- verify local identity/trust state as needed

### 3. Omegon engine becomes available
- launch or attach to bundled Omegon
- obtain startup/discovery metadata
- verify Omegon version and control-plane schema compatibility

### 4. Session host becomes active
- establish desktop-local host session
- populate session cache
- expose remote relay if enabled

### 5. UI enters normal operation
- local desktop UI becomes live
- phone relay sessions may attach through Styrene

## Health model

Auspex should supervise subsystem health explicitly.

### Health domains
- UI shell health
- Omegon process health
- Styrene runtime health
- local host session health
- remote relay session health

### Principle
A subsystem failure should not be treated as generic app failure if the UI can still explain and recover from it.

## Compatibility model in the embedded runtime

Even when Auspex bundles Omegon and Styrene, compatibility checks still matter.

### Why
- development builds can drift
- override modes may exist
- external attach modes may still be supported later
- packaging mistakes happen

### Required checks
At minimum verify:
- bundled Omegon version
- control-plane schema version
- Styrene runtime availability

## External attach as a secondary mode

The primary desktop model should be:
- bundled, managed, first-party subsystems

External attach can still exist later, but it should be treated as a secondary mode, not the foundation of the product.

## Remote phone implication

This model strengthens the phone strategy.

Because Auspex owns:
- the bundled Omegon engine
- the bundled Styrene runtime
- the host session

it becomes the natural remote authority for:
- phone transcript relay
- command forwarding
- state projection
- connection and compatibility state

## Guiding rule

Auspex should be the sovereign product shell while Omegon and Styrene remain specialized embedded subsystems.
