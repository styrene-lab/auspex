# Remote Connection Model

## Purpose

Define how Auspex should connect beyond the same-machine local process case.

The immediate goal is not full mobile-local execution. The immediate goal is a trustworthy remote UI model, with the Auspex desktop client acting as the first concrete bridge point for remote phone clients.

## Core decision

### Remote UI comes before on-device mobile inference

Auspex should become a remote-first control client before it becomes a local-first mobile executor.

That means the near-term mobile story is:
- phone as a remote UI
- desktop Auspex as the local host/bridge point
- Omegon as the execution/control-plane backend

On-device MLX inference on iOS is still strategically interesting, but it is a later execution-backend concern, not the first mobile connection concern.

## Immediate connection topology

### Phase 1 topology

```text
Omegon process <-> Auspex desktop <-> phone Auspex client
```

The desktop Auspex client is the thing the phone connects to for the time being.

This keeps the first remote-client model simple:
- Omegon can stay local to the desktop host
- the desktop app owns local process launch/attach
- the phone talks to a user-facing client layer rather than directly to Omegon's current localhost-oriented control plane

## Why this is the right first remote model

### 1. The current Omegon control plane is local-first
Omegon's existing web/control-plane surface is strong enough for a local client, but not yet hardened enough to be treated as an internet-facing or phone-facing remote endpoint directly.

### 2. Desktop is the natural bridge for now
Desktop already owns:
- repository context
- local process launch
- local auth/token handling
- richer debugging and operator recovery

### 3. Phone value is mostly control and awareness first
The first mobile value is:
- transcript
- prompt input
- run visibility
- cancellation
- work/graph inspection

not full local agent execution.

## Connection modes

### Mode A — Managed local
Desktop Auspex launches or attaches to a local Omegon process.

### Mode B — Remote phone attachment
Phone Auspex attaches to a desktop Auspex instance that is already connected to Omegon.

This means the first remote protocol boundary is:
- **phone <-> desktop Auspex**

not:
- **phone <-> Omegon directly**

## Transport decision

### Use Styrene as the remote connection and comms layer

For the remote phone-to-desktop link, the right direction is to use Styrene as the connection and communications protocol.

That is better than inventing an ad-hoc second remote transport for Auspex.

The important boundary distinction is:
- Omegon <-> desktop Auspex: current JSON HTTP/WS control-plane
- desktop Auspex <-> phone Auspex over Styrene: semantic relay protocol with transport-native encoding, likely MessagePack over LXMF

## Why Styrene fits this role

### Trust and identity
Styrene already represents the broader product direction for trusted communications and identity. Reusing it for Auspex remote connectivity avoids inventing one-off pairing/auth behavior that would later need to be replaced.

### Product alignment
The phone-to-desktop remote link is not just raw transport. It is part of the wider Black Meridian / Styrene ecosystem story around:
- trusted links
- operator identity
- remote control
- later collaboration

### Future path
Using Styrene now for remote UI does not block later direct Omegon remote attach or MLX-enabled mobile modes. It creates a cleaner path toward both.

## Scope of Styrene use in the first remote phase

### In scope
- desktop Auspex reachable over Styrene
- phone Auspex attaching to desktop Auspex over Styrene
- remote transcript and command relay
- remote state/view updates
- remote cancellation

### Not required yet
- full peer-to-peer collaborative editing
- mesh-native multi-operator session sharing
- remote direct Omegon exposure over Styrene without the desktop app in the middle
- mobile-local inference execution

## Relay model

Desktop Auspex should act as a relay/control host for the phone client.

### Desktop responsibilities
- launch or attach to Omegon
- perform compatibility handshake with Omegon
- maintain local state cache
- expose a phone-facing remote session over Styrene
- relay prompts, cancellations, and relevant control actions
- relay snapshot and event data back to the phone

### Phone responsibilities
- connect to desktop Auspex over Styrene
- authenticate/pair through the Styrene trust model
- render Simple mode first
- optionally expose Power mode surfaces at reduced density

## UI implications

### Phone first priority
The phone UI should prioritize Simple mode capabilities first:
- transcript
- prompt input
- current activity
- connection state
- cancel
- compact work summary

### Power mode on phone
Power mode should still exist, but it should be a projection of the same contract with mobile-appropriate density, not a desktop layout shrunk down mechanically.

## Compatibility implications

The desktop Auspex instance should remain the component that verifies Omegon compatibility directly.

That means the phone client does not initially need to implement the full Omegon compatibility handshake itself. Instead, it trusts the desktop Auspex host session, which has already validated Omegon.

This simplifies the first remote path substantially.

## Future evolution

### Later path A — direct remote Auspex <-> Omegon
Once Omegon's remote-facing contract and trust model mature, a phone client could attach more directly.

### Later path B — iOS MLX execution
Once remote UI is genuinely useful, iOS-local MLX inference can be added as an optional execution backend or offline mode.

That should be layered in after the remote connection and control-plane model are stable.

## Guiding rule

For mobile, solve trusted remote control first. Solve on-device execution second.
