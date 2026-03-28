# Styrene Relay Session Model

## Purpose

Define the first remote session boundary between:
- phone Auspex client
- desktop Auspex host

This is the next layer above the remote-connection model. It specifies what the desktop host relays, what the phone can ask for, and what state model the phone should see.

## Scope

This document is about:
- phone <-> desktop Auspex session behavior
- remote state and command relay over Styrene

It is not about:
- direct phone <-> Omegon attach
- full collaborative multi-operator sessions
- final Styrene transport implementation details

## Encoding note

This relay layer should be defined semantically, not as a JSON-first wire contract.

The Omegon control-plane remains JSON at the HTTP/WS boundary, but the Styrene relay path should assume transport-native encoding — most likely MessagePack over LXMF — rather than inheriting JSON assumptions from the local control-plane.

## Core decision

### The phone sees a desktop-hosted session model, not raw Omegon internals

The phone client should not initially talk to Omegon's raw control plane.

Instead, the desktop Auspex host should:
- connect to Omegon
- validate Omegon compatibility
- maintain a normalized session cache
- project a remote-safe session model to the phone over Styrene

This gives the phone a simpler and more stable surface.

## Relay responsibilities

### Desktop Auspex host
The desktop host is authoritative for:
- local Omegon process launch or attach
- compatibility handshake with Omegon
- current control-plane cache
- WebSocket event subscription
- command forwarding to Omegon
- remote session exposure over Styrene

### Phone Auspex client
The phone client is responsible for:
- connecting to the desktop host over Styrene
- authenticating/pairing through the host's trust model
- rendering transcript and state projections
- issuing allowed user commands to the host

## Relay data model

### Host session summary
The host should expose a session summary containing at least:
- host label / identity
- connection state to Omegon
- compatible/incompatible status
- current repository/session label
- current mode capability (Simple/Power available)

### Remote session state
The phone-facing state should be derived from the desktop's cached `ControlPlaneStateV1`, but it does not need to expose every field immediately.

#### Minimum remote state
- compact session info
- transcript state
- current activity state
- focused work summary
- compact OpenSpec / cleave summaries
- compatibility and health state

#### Optional remote state
- graph data
- richer Session screen data
- deeper Power mode details

## Remote command set

### Required commands
The phone should be able to ask the desktop host to:
- submit a prompt
- cancel the current run
- request a fresh snapshot
- switch between Simple and Power mode locally on the phone

### Optional early commands
- run a slash command
- request graph refresh
- request session details

### Not required yet
- launch arbitrary local tools from the phone directly
- manage local Omegon process settings remotely in a broad way
- edit desktop-side files directly through the relay

## Event relay model

The host should relay a filtered event stream rather than every possible internal event indiscriminately.

### Required relayed events
- transcript events
- system notifications
- current activity/tool summary changes
- focused work changes
- compatibility/degradation changes
- run completion / cancellation

### Optional relayed events
- full tool lifecycle detail
- thinking stream
- raw event log

These are better suited to later Power mode expansion.

## State synchronization model

### Host as source of truth
The desktop host is the state authority for the remote phone session.

The phone should maintain a local cache for UI responsiveness, but it should treat the desktop host as canonical.

### Reconnect behavior
On reconnect, the phone should:
1. re-establish the Styrene session
2. request the latest remote snapshot
3. resume event subscription

The phone should not try to reconstruct state from missed events alone.

## Error handling model

### If the desktop loses Omegon
The phone should see a host-session degradation state, not a silent stall.

### If the phone loses the desktop host
The phone should show a reconnecting / disconnected remote-session state, not a generic Omegon failure.

This distinction matters because the first remote path has two layers:
- phone <-> desktop host
- desktop host <-> Omegon

## Compatibility model for remote sessions

The desktop host should perform the Omegon compatibility handshake.

The phone should initially trust the desktop host's compatibility result rather than re-implementing full Omegon-version negotiation itself.

That means the first phone-visible compatibility state is:
- host session compatible
- host session incompatible
- host session unknown / reconnecting

## Screen implications

### Phone Simple mode should consume
- remote session summary
- transcript
- activity summary
- compact work state
- cancel action

### Phone Power mode may later consume
- graph projection
- deeper work state
- richer system/session details

## Guiding rule

The remote phone client should attach to a stable desktop-hosted session abstraction, not directly inherit Omegon's raw control-plane complexity.
