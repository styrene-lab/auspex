+++
title = "Nested Agent Delegation Graph"
tags = ["architecture","agents","delegation","mesh","styrene","mqtt"]
+++

# Nested Agent Delegation Graph

## Problem
Auspex supervises multiple attached/owned Omegon instances. Each Omegon is an agent runtime and may create delegate or cleave subagents. The model must preserve this hierarchy without assuming all descendants are directly controlled by Auspex or share a transport.

## Core decision: graph, not recursive ownership
Represent orchestration as a directed provenance graph. A node is an agent runtime identity; a run is an execution; an edge records who requested a child run. `parent_run_id` gives the immediate causal parent and `root_operation_id` groups the full operator-visible operation. This supports nested delegation while allowing remote descendants to be observed without pretending Auspex owns their process lifecycle.

### Identities
- `agent_id`: stable logical agent identity, independent of process and transport.
- `runtime_id`: one concrete Omegon process/incarnation; changes on restart.
- `run_id`: one execution delegated to an agent runtime.
- `root_operation_id`: end-to-end operator objective spanning all descendants.
- `parent_run_id`: immediate causal parent, absent only at the root.
- `command_id`: idempotent command delivery identity; not a run or agent identity.

### Edge semantics
Each delegation edge carries:
- requester agent/runtime/run
- child agent/runtime/run
- relationship: `delegate | cleave_child | handoff | operator_attach`
- authority grant: tools, path scope, network policy, budget, max depth
- transport route metadata kept outside the signed protocol payload

## Authority model
Authority only attenuates down the graph. A child grant MUST be a subset of the effective parent grant. A child may not increase tool, filesystem, network, time, token, or recursion allowances. Auspex can issue policy at a root or managed-runtime boundary; Omegon enforces grants for direct children and reports effective policy. `max_depth` is policy, not a hardcoded V1 ban. The previous non-recursive assumption is rejected.

Ownership is explicit:
- `owned`: Auspex launched and may terminate the runtime.
- `attached`: Auspex has authenticated control but not process ownership.
- `observed`: telemetry/provenance only; no control claim.
- `delegated`: lifecycle authority belongs to the immediate parent runtime, bounded by inherited grant.

Cancellation defaults to subtree intent but is hop-by-hop: cancelling a run asks its immediate runtime to cancel direct child runs recursively. Auspex records acknowledgements and confirmed termination per node; it must not claim descendant death from an ancestor acknowledgement.

## Protocol envelope
Transport-neutral canonical CBOR/COSE payload:

```text
AgentEnvelope v1
  message_id: uuid/16 bytes
  kind: command | event | result | receipt | snapshot
  root_operation_id
  run_id
  parent_run_id?
  source: { agent_id, runtime_id }
  target: { agent_id, runtime_id? }
  sequence: u64               # monotonic per source runtime + run
  created_at
  expires_at?
  schema: URI/name + version
  payload: typed CBOR value
  signature / COSE protection
```

The protocol MUST NOT encode LXMF, MQTT, WebSocket, or topic names. Transport adapters map this envelope onto a bearer.

## Delivery semantics
Application delivery is at-least-once with idempotent effects. `message_id` deduplicates commands/events; `run_id` identifies execution; `sequence` orders one producer stream. Do not promise global ordering across agents or exactly-once execution across process crashes.

Receipts have layers:
1. bearer accepted (optional transport evidence)
2. target runtime accepted/deduplicated command
3. method outcome/event

Store-and-forward expiry is explicit. A snapshot reconciles gaps after reconnect. Event logs are append-only per runtime/run; snapshots compact current state.

## Styrene happy path
Use Styrene wire v2 framing and CBOR. Existing `StyreneMessage.request_id` can carry/derive `message_id`, but agent-level identity and causality remain in the payload. Small control/events use direct Styrene/RNS messages; large artifacts use resource transfer and content references. LXMF is a bearer option for delay-tolerant/offline delivery, not the agent protocol itself.

The current Styrene wire offers 16-byte request correlation, CBOR, propagation ingest/fetch/delete, resource transfer, and identity derivation for per-agent signing keys. New message types or a generic application message type will be needed for agent envelopes; avoid consuming many fixed enum values per agent method.

## MQTT interoperability
MQTT 5 adapter mapping:
- command topic: `styrene/v1/agents/{target_agent}/commands`
- event topic: `styrene/v1/operations/{root_operation}/events`
- retained state topic: `styrene/v1/agents/{agent}/state`
- Response Topic + Correlation Data map request/reply and `message_id`
- Message Expiry maps `expires_at`
- persistent sessions support reconnect delivery
- QoS 1 is the default; application dedup remains mandatory
- QoS 2 may be offered but does not replace application idempotency or crash-safe effects

MQTT ACLs must bind authenticated principal to allowed agent/topic scope. Topic paths are adapter metadata and never signed protocol identity.

## Reconciliation
On attach/reconnect Auspex requests a graph snapshot containing runtime incarnation, active/recent runs, parent/root links, effective grants, last sequence per run, and terminal result references. Auspex then subscribes from sequence watermarks where supported. Unknown parents produce orphan nodes, not fabricated ancestry. Cycles are rejected. Runtime restart creates a new `runtime_id`; logical `agent_id` remains stable.

## UI projection implications (not implementation yet)
Primary view: root operations and directly managed agents. Expandable descendants show nested delegates/cleave children. Every node displays ownership/control level, runtime incarnation, effective policy, transport health, and independently confirmed lifecycle state. Graph and tree are projections of the same provenance model.

## Open questions
- [assumption] Styrene identity manifests can bind `agent_id` to a signing key and runtime attestations without adding a separate PKI.
- How are root operation IDs minted when the operator directly instructs an attached Omegon outside Auspex?
- Which subtree cancellation semantics are mandatory across transports that cannot provide timely acknowledgement?
- Should snapshots include completed descendants indefinitely or only content-addressed summaries plus retention policy?
- What generic Styrene message type allocation avoids coupling the fixed wire enum to every agent protocol revision?
