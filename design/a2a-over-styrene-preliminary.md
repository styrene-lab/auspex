+++
title = "A2A over Styrene — Preliminary Protocol and IPC Design"
tags = ["architecture","a2a","styrene","ipc","mesh","agents"]
+++

# A2A over Styrene — Preliminary Protocol and IPC Design

## Status

Preliminary architecture. This design adopts Linux Foundation A2A task semantics and the official Rust `a2a-lf` core types behind a Styrene-owned facade. It does not adopt an external control-plane runtime.

This document supersedes the bespoke task-envelope portions of [[Nested Agent Delegation Graph]]. The delegation graph, authority attenuation, and transport-neutrality decisions remain.

## Goals

1. Make Omegon agents interoperable through A2A v1 semantics.
2. Keep Styrene/RNS the preferred decentralized transport.
3. Reuse existing Styrene wire, IPC, telemetry, propagation, resource, identity, and MQTT facilities.
4. Support nested delegate and cleave children without claiming that Auspex directly owns every descendant.
5. Avoid encoding transport addresses or broker topics in the signed A2A/domain payload.
6. Preserve HTTP/JSON-RPC A2A interoperability through adapters and the official TCK.

## Non-goals

- Reimplementing A2A Task, Message, Artifact, Agent Card, or task-event schemas.
- Requiring HTTP, MQTT, NATS, LXMF, or any single transport.
- Exactly-once effects across process crashes.
- Adopting Temporal, `ra2a`, or another full orchestration runtime as a core dependency.
- Exposing transport SDK types throughout Auspex or Omegon.

## Adopted dependencies

### A2A

Use the official Rust SDK core crate:

```toml
[dependencies]
a2a = { package = "a2a-lf", version = "0.2" }
```

Pin the exact resolved version in `Cargo.lock`. The dependency is wrapped by a new `styrene-a2a` crate; applications do not depend directly on its public types.

Adopt directly:

- `AgentCard` and skills
- `Task` and standard task states
- `Message` and `Part`
- `Artifact`
- task status and artifact update events
- protocol errors
- extension declarations

### Authorization

Use `biscuit-auth` for offline attenuation of delegation grants. Authentication remains a Styrene identity/transport concern.

### Observability

Use W3C Trace Context and OpenTelemetry GenAI conventions. Trace IDs do not replace A2A task IDs or Styrene message IDs.

## Layer model

```text
┌─────────────────────────────────────────────────────────────┐
│ Auspex/Omegon domain                                       │
│ graph projection, policy, execution, operator control      │
├─────────────────────────────────────────────────────────────┤
│ styrene-a2a facade                                         │
│ A2A types + Styrene delegation extension + validation      │
├─────────────────────────────────────────────────────────────┤
│ Styrene application envelope                               │
│ identity, causality, expiry, sequencing, grant, payload    │
├─────────────────────────────────────────────────────────────┤
│ Bindings                                                   │
│ mesh wire | local IPC | LXMF | MQTT | HTTP/JSON-RPC A2A    │
├─────────────────────────────────────────────────────────────┤
│ Bearers                                                    │
│ RNS links/resources | Unix socket | MQTT 5 | HTTP/SSE       │
└─────────────────────────────────────────────────────────────┘
```

A2A owns task semantics. Styrene owns transport binding, decentralized identity, nested delegation provenance, authority attenuation, reconciliation, and delivery evidence.

## Crate boundaries

### `styrene-a2a`

A new transport-independent library in `styrene-rs/crates/libs/styrene-a2a`.

Responsibilities:

- selectively re-export stable facade types rather than the entire SDK;
- convert between Styrene IDs and A2A string IDs;
- define and validate the Styrene delegation extension;
- encode/decode the compact CBOR profile;
- validate graph causality and grant attenuation;
- provide JSON fixtures compatible with standard A2A endpoints;
- contain all dependency-specific conversions.

Proposed API:

```rust
pub struct AgentTask { /* private a2a backing value */ }
pub struct AgentCard { /* private a2a backing value */ }
pub enum AgentEvent { Status(...), Artifact(...) }

pub struct StyreneDelegationExtension {
    pub root_operation_id: RootOperationId,
    pub parent_task_id: Option<TaskId>,
    pub source: AgentRuntimeRef,
    pub target: AgentRuntimeSelector,
    pub relationship: DelegationRelationship,
    pub ownership: ControlClass,
    pub remaining_depth: u16,
    pub grant: DelegationGrantRef,
    pub parent_close_policy: ParentClosePolicy,
    pub traceparent: Option<String>,
}
```

### `styrene-a2a-transport`

May initially be a module of `styrene-a2a`; split only after two bindings exist.

```rust
#[async_trait]
pub trait A2aTransport {
    async fn send(&self, target: &TransportTarget, envelope: AgentEnvelope)
        -> Result<DeliveryEvidence, TransportError>;
    async fn subscribe(&self, selector: SubscriptionSelector)
        -> Result<AgentEnvelopeStream, TransportError>;
    async fn reconcile(&self, runtime: &AgentRuntimeRef, watermark: Watermark)
        -> Result<GraphSnapshot, TransportError>;
}
```

### Applications

- Omegon implements the A2A task executor and direct-child supervisor.
- Auspex stores/project the provenance graph and controls owned/attached roots.
- Neither application owns wire framing or SDK conversion.

## Identity and causality

Keep these distinct:

| ID | Owner | Purpose |
|---|---|---|
| `agent_id` | Styrene identity profile | Stable logical agent |
| `runtime_id` | Runtime | Process incarnation; changes on restart |
| A2A `task_id` | Serving agent | One unit of work |
| A2A `context_id` | Serving agent | Related interactions/multi-turn context |
| `root_operation_id` | Root supervisor | Whole operator-visible operation |
| `parent_task_id` | Delegating runtime | Immediate causal parent |
| `message_id` | Producer | Deduplication and delivery |
| IPC/mesh `request_id` | Transport endpoint | Request/response correlation |
| W3C trace/span IDs | Telemetry | Observability only |

A2A `context_id` is not sufficient for nested provenance: related follow-up tasks may share context without being parent/child. The Styrene extension therefore retains `root_operation_id` and `parent_task_id`.

## Styrene application envelope

A2A objects are carried in a versioned transport-neutral envelope encoded as canonical CBOR:

```rust
pub struct AgentEnvelope {
    pub profile_version: u16,
    pub message_id: [u8; 16],
    pub kind: AgentEnvelopeKind,
    pub source: AgentRuntimeRef,
    pub target: AgentRuntimeSelector,
    pub sequence: u64,
    pub created_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub payload_schema: String,
    pub a2a_payload: Vec<u8>,
    pub authorization: Option<Vec<u8>>, // Biscuit token
    pub traceparent: Option<String>,
}
```

`a2a_payload` is canonical CBOR derived from the A2A facade. HTTP adapters convert the same facade to canonical A2A JSON/protobuf. Do not sign serialized MQTT topics or RNS routes; sign/protect the envelope.

`message_id` is copied into the existing 16-byte Styrene mesh/IPC `request_id` where a one-request/one-response correlation exists. It remains independently present in the envelope because pushed events use a zero IPC request ID and because one command may produce multiple events.

## Existing Styrene mesh wire fit

Current `styrene-mesh` wire v2 already provides:

```text
namespace[11] | version[1] | message_type[1] | request_id[16] | payload
```

Payloads use CBOR. This is a good carrier and does not need a new frame format.

### Message allocation

Do not allocate one fixed `StyreneMessageType` per A2A method. Add a small generic range:

```text
0x70 AgentEnvelope       command/message/event/result/snapshot in payload
0x71 AgentReceipt        target-runtime acceptance/dedup/rejection
0x72 AgentReconcile      graph/event-log reconciliation request
0x73 AgentReconcileResult
```

The existing `0x70–0x7F` mesh range is presently unallocated in `styrene-mesh`; verify the Python protocol registry before reserving it. If cross-language allocation conflicts, use another registry-approved contiguous block.

`request_id` mapping:

- request/reply: copy `message_id` into frame `request_id`;
- response: preserve request ID using `StyreneMessage::with_request_id`;
- streamed task events: fresh `message_id`; no false request/reply implication;
- snapshots: request ID correlates the reconciliation exchange.

### Existing message reuse

| Existing facility | A2A use |
|---|---|
| `CapabilitiesRequest/Response` | Bootstrap indication that A2A and extension versions are supported; not a replacement for Agent Card |
| `Announce` | Advertise an agent service/card reference |
| `ResourceAvailable/ChunkRequest/ChunkResponse` | Large A2A artifacts and Agent Cards by content reference |
| `PropagationIngest/Fetch/Delete` | Carry LXMF-encoded agent envelopes for offline recipients |
| `Error` | Transport/framing failure only; A2A failures remain A2A protocol/task errors |

Do not repurpose `Exec`, `Chat`, or `FileOffer` as A2A task operations. They are legacy/domain commands and can later be exposed as agent skills through adapters.

## Existing local IPC fit

The IPC server currently uses:

```text
length | message_type | request_id[16] | MessagePack map payload
```

It has query, command, subscription, result/error, and pushed event categories. Pushed events currently use an all-zero request ID.

### Proposed IPC additions

Allocate a coherent new block rather than overloading existing commands:

```text
Requests/commands
  QueryAgentCard
  QueryAgentTask
  QueryAgentGraph
  CmdAgentSendMessage
  CmdAgentCancelTask
  CmdAgentReconcile

Subscriptions
  SubAgentTasks
  SubAgentGraph

Events
  EventAgentTask
  EventAgentArtifact
  EventAgentGraph
  EventAgentDelivery
```

Exact byte assignments require coordinated update of the Rust/Python IPC registry and compatibility fixtures. The current ranges are dense; do not choose values independently in one implementation.

### IPC payload strategy

Short term: IPC payload remains MessagePack for compatibility, with one field carrying the A2A JSON-compatible object or compact envelope bytes.

Recommended payload map:

```text
profile_version
message_id
encoding: "a2a-json" | "styrene-a2a-cbor"
envelope: binary/string
```

Long term: IPC may gain a generic extension message type, but changing the frame format is unnecessary.

### IPC trait additions

Extend `styrene-ipc` with a focused trait rather than adding methods to unrelated messaging/fleet traits:

```rust
#[async_trait]
pub trait DaemonAgents {
    async fn get_agent_card(&self, selector: AgentSelector) -> Result<AgentCardData, IpcError>;
    async fn send_agent_message(&self, request: AgentSendRequest) -> Result<TaskHandle, IpcError>;
    async fn get_agent_task(&self, task: TaskSelector) -> Result<AgentTaskData, IpcError>;
    async fn cancel_agent_task(&self, task: TaskSelector) -> Result<(), IpcError>;
    async fn get_agent_graph(&self, root: RootOperationId) -> Result<GraphSnapshot, IpcError>;
    async fn subscribe_agent_events(&self, filter: AgentEventFilter)
        -> Result<broadcast::Receiver<DaemonEvent>, IpcError>;
}
```

Add `DaemonEvent::Agent { event: AgentDaemonEvent }`. Local UI clients consume this projection; they do not parse raw RNS/LXMF frames.

## Service discovery fit

`styrene-telemetry::ServiceAnnouncement` already carries:

- `service_type`
- 16-byte node identity
- status
- up to 256 bytes of opaque CBOR metadata
- timestamp

Use:

```text
service_type = "a2a-agent"
```

Metadata should be a small `AgentServiceHint`, not the full Agent Card:

```rust
pub struct AgentServiceHint {
    pub agent_id: AgentId,
    pub runtime_id: RuntimeId,
    pub profile_version: u16,
    pub a2a_version: String,
    pub card_content_id: Option<[u8; 32]>,
    pub card_digest: [u8; 16],
    pub capability_flags: u32,
}
```

The 256-byte metadata cap makes a content-addressed Agent Card reference preferable. Fetch the card through Styrene content/resource transfer, HTTP when advertised by the card, or an IPC query for local clients.

`styrene-services::protocol_registry` is the natural daemon extension point for registering the generic agent-envelope handler. `node_store` can retain service hints. A dedicated `agent_service` should own cards, task routing, graph snapshots, and event publication.

## Identity and authorization fit

Existing `styrene-rns` identities support signing, verification, encryption, and stable 16-byte identity hashes. Existing LXMF wire messages can be signed and verified.

Preliminary mapping:

```text
agent_id   = stable URI/name bound to an RNS public identity
runtime_id = random UUID plus signed runtime attestation
node_identity[16] = transport discovery identity hash
```

Do not make the 16-byte RNS node hash the only logical `agent_id`; one node may host multiple agents and one logical agent may restart.

Biscuit grant tokens are included in `AgentEnvelope.authorization`. The verifier binds facts to the authenticated source identity, target agent, task/root operation, expiry, and remaining depth. Each child appends restrictions before delegating.

Open assumption: define the exact signed binding between Agent Card, `agent_id`, RNS identity, and runtime attestation. This needs a separate identity profile review.

## LXMF and propagation fit

LXMF provides opportunistic, direct, propagated, and resource delivery plus signed messages. It is appropriate for delayed/offline A2A traffic.

Binding:

- encode one `AgentEnvelope` as LXMF content/field payload;
- use direct delivery when a path exists;
- use propagated delivery for offline tasks/events with explicit expiry;
- use LXMF resource mode or Styrene content references for large artifacts;
- retain application `message_id` and deduplicate after LXMF delivery;
- never infer A2A task completion from LXMF transport delivery.

The existing `PropagationIngest/Fetch/Delete` flow can carry raw signed LXMF bytes unchanged. Fetch/delete acknowledgement is propagation-store lifecycle, not target-runtime command acceptance.

## Resource and artifact fit

A2A Artifacts may contain text, files, or structured data. Small parts remain inline. Large parts use a Styrene extension data part:

```text
styrene.content_ref = {
  content_id,
  media_type,
  byte_length,
  manifest_hash,
  optional_name
}
```

`ResourceAvailable`, `ChunkRequest`, and `ChunkResponse` already support BLAKE3 content IDs, manifests, and chunk distribution. RNS resource transfer is also available for link-oriented bulk transfer.

The standard HTTP A2A adapter materializes or streams the referenced content as a standard FilePart/DataPart when crossing into a peer that does not support the Styrene extension.

## MQTT fit

`styrene-mqtt` already has:

- operator/service/instance/event topic hierarchy;
- typed envelopes and metadata;
- QoS policy;
- embedded broker ownership in Auspex;
- typed subscriptions.

Do not introduce a competing topic tree immediately. Map agent events into the existing schema:

```text
styrene/{operator}/omegon/{runtime_id}/events/agent.task.updated
styrene/{operator}/omegon/{runtime_id}/events/agent.artifact.updated
styrene/{operator}/omegon/{runtime_id}/events/agent.graph.updated
styrene/{operator}/omegon/{runtime_id}/events/agent.delivery.updated
```

Commands should use a distinct command namespace only after `styrene-mqtt::TopicAddress` supports a channel/kind other than `events`. Until then MQTT is event projection, not the authoritative control path.

Correct the current QoS documentation: MQTT QoS 2 is broker delivery evidence, not exactly-once application execution. Agent lifecycle events still require `message_id` deduplication.

For a full MQTT control binding, add MQTT 5 properties:

- Correlation Data = `message_id`
- Response Topic = runtime-specific reply route
- Message Expiry = envelope expiry
- QoS 1 default

## A2A extension

Declare one versioned A2A extension URI:

```text
https://styrene.io/a2a/extensions/delegation/v1
```

The extension payload carries only semantics absent from core A2A:

```text
root_operation_id
parent_task_id
source agent/runtime
relationship
control class
effective grant reference
remaining depth
parent-close policy
traceparent
sequence watermark
```

Do not duplicate core Task state, context ID, messages, artifacts, or standard errors.

## Graph reconciliation

A2A task polling is necessary but insufficient for rebuilding nested provenance after reconnect. Define a Styrene reconciliation payload:

```rust
pub struct GraphSnapshot {
    pub snapshot_id: [u8; 16],
    pub source: AgentRuntimeRef,
    pub generated_at_ms: u64,
    pub roots: Vec<RootOperationSummary>,
    pub tasks: Vec<TaskGraphNode>,
    pub edges: Vec<DelegationEdge>,
    pub watermarks: Vec<RunWatermark>,
    pub retention: SnapshotRetention,
}
```

Rules:

- unknown parent produces an orphan marker;
- cycles are rejected;
- runtime restart creates a new `runtime_id`;
- snapshots contain standard A2A task summaries plus Styrene graph metadata;
- large snapshots are transferred by content reference;
- incremental events resume from per-runtime/task sequence watermarks where available.

## Cancellation and parent-close policy

Use explicit policies inspired by durable workflow systems:

```text
request_cancel  propagate cooperative cancellation to direct children
terminate       require owned runtime/process termination where authority permits
abandon         children continue under immediate supervisor
detach          child becomes a separately supervised root
await           parent may not complete while required children are non-terminal
```

Core A2A cancellation applies to one task. Subtree semantics are a Styrene extension executed hop-by-hop. Every node reports independent acknowledgement and terminal confirmation.

## Compatibility with current Auspex/Omegon control work

The existing managed-command receipt work remains useful as a local authenticated control binding, but it should become an adapter rather than the canonical agent protocol:

| Existing concept | Destination |
|---|---|
| `ManagedRunId` | A2A task ID facade or local projection key |
| `ManagedCommandId` | `AgentEnvelope.message_id` / delivery idempotency |
| `ManagedRunState` | Projection from A2A Task state plus delivery/transport state |
| delegate dispatch/get/result/cancel | A2A send/get/cancel/task events behind an Omegon compatibility adapter |
| command receipt | Styrene `AgentReceipt`, separate from A2A task outcome |
| effective policy | Biscuit/effective-grant summary in Styrene extension |

Do not delete the current path before an A2A adapter passes equivalent lifecycle tests. Migrate by dual projection: existing delegate control remains execution plumbing while A2A becomes the public semantic surface.

## Preliminary flow

```text
1. Omegon announces service_type=a2a-agent with card content reference.
2. Auspex retrieves and validates Agent Card.
3. Auspex creates A2A Message + Styrene delegation extension.
4. styrene-a2a wraps it in AgentEnvelope with Biscuit grant.
5. Binding sends AgentEnvelope over local IPC, RNS direct, or LXMF.
6. Target validates identity, expiry, dedup ID, extension, and grant.
7. Target emits AgentReceipt accepted/duplicate/rejected.
8. Target creates/updates standard A2A Task.
9. Task status/artifact events return as AgentEnvelope events.
10. Child delegation creates another A2A Task with same root operation and immediate parent task.
11. Reconnect invokes GraphSnapshot reconciliation, then resumes events.
12. Large artifacts resolve through Styrene content/resource transfer.
```

## Implementation phases

### Phase 0 — dependency and conformance spike

- Add a standalone spike crate using `a2a-lf` core only.
- Round-trip Agent Card, Message, Task, status event, artifact event through JSON and CBOR.
- Verify arbitrary extension preservation.
- Measure dependency/MSRV impact.
- Run the A2A TCK against a minimal HTTP adapter.
- Compare fixtures with an independent A2A v1 implementation.

Exit: no patched/forked A2A core types required.

### Phase 1 — `styrene-a2a` facade

- IDs and A2A conversions.
- Delegation extension.
- AgentEnvelope CBOR profile.
- Biscuit grant reference and validation interface.
- Golden fixtures.

### Phase 2 — local IPC adapter

- `DaemonAgents` trait.
- coordinated IPC message allocations.
- agent event subscription.
- Omegon compatibility adapter over existing delegate control.

### Phase 3 — mesh adapter

- registry-approved generic mesh message types.
- service announcement/card discovery.
- direct RNS send and receipt.
- graph reconciliation.
- resource-backed artifacts.

### Phase 4 — delayed and broker adapters

- LXMF direct/propagated binding.
- MQTT event projection, then optional command binding.
- optional NATS/JetStream adapter.

### Phase 5 — application migration

- Omegon native A2A executor and child-task reporting.
- Auspex graph store and supervisor.
- retire bespoke public delegate DTOs after parity and migration tests.

## Tests required before commitment

1. Official A2A JSON fixture compatibility.
2. JSON ↔ facade ↔ canonical CBOR ↔ facade round trip.
3. Unknown extension preservation.
4. Nested task parent/root reconstruction.
5. Cycle and authority-escalation rejection.
6. Duplicate message receipt without duplicate effects.
7. IPC request/result correlation and pushed-event behavior.
8. Direct RNS and propagated LXMF delivery of the same envelope.
9. Artifact content-reference integrity.
10. Runtime restart and graph reconciliation.
11. Cross-transport identity consistency.
12. A2A TCK MUST-level conformance for HTTP adapter.

## Decisions

1. Adopt A2A v1 semantics and official Rust core types behind `styrene-a2a`.
2. Keep the existing Styrene mesh and IPC frame formats.
3. Add generic agent-envelope message families, not one type per A2A operation.
4. Use A2A Agent Cards with content-addressed discovery hints.
5. Use Styrene identities for authentication and Biscuit for attenuated authority.
6. Keep task, delivery, transport, and trace identities separate.
7. Treat MQTT initially as event projection; Styrene/RNS remains the preferred control bearer.
8. Preserve current Omegon delegate control as migration plumbing, not the future semantic contract.

## Open questions

- [assumption] `a2a-lf` preserves arbitrary extension data through all required JSON/protobuf conversions.
- Confirm unallocated mesh type codes against every language implementation before reservation.
- Choose whether canonical CBOR is deterministic enough through Serde conversion or needs a normative field-number profile.
- Define Agent Card signing/binding to Styrene identities.
- Define whether graph snapshots are a Styrene extension method, an A2A skill, or both.
- Determine retention and pruning for completed descendant summaries.
- Determine whether local IPC transports raw envelope bytes or a stable boundary DTO in the first migration release.
