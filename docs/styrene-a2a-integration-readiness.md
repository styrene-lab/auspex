# Styrene A2A integration readiness

Status: waiting on upstream implementation
Owner: Auspex managed-agent backend
Upstream: `../styrene-rs`, branch `feat/a2a-integration-foundation`

## Purpose

Prepare Auspex to adopt Styrene's A2A implementation without coupling to an in-progress worktree or duplicating A2A domain semantics. This document is an integration gate, not an instruction to add a path dependency now.

## Boundary decision

Auspex keeps its current Omegon supervision contract for local process control. Styrene A2A becomes the portable task/message/event and delegation-graph model. The two layers meet through an adapter owned by Auspex:

```text
Omegon supervisor DTOs ─┐
                        ├─ Auspex ManagedAgentPort ─ canonical run/graph projection
Styrene A2A service  ───┘
```

The adapter must not leak A2A SDK structs into `controller.rs`, Dioxus props, or transport code. `auspex-core` consumes a narrow internal port; a future `styrene_a2a_adapter` module performs conversions.

## Source-of-truth ownership

| Concern | Owner |
|---|---|
| Agent Card, A2A message/task/status/artifact | `styrene-a2a` / official A2A SDK |
| Stable agent identity, runtime incarnation, root/parent task provenance | Styrene delegation extension |
| Durable task/event graph, sequence watermarks, reconciliation | `styrene-services::agent` |
| Local daemon operations and projections | `styrene-ipc::DaemonAgents` |
| Omegon process dispatch, receipt, polling, cancellation transport | existing Auspex/Omegon supervisor contract |
| Operator policy, instance routing, audit timeline | Auspex |
| UI projection | Auspex, derived from the canonical port only |

Auspex must not create alternative definitions of A2A `Task`, `TaskState`, `Message`, `Artifact`, Agent Card, or delegation ancestry.

## Internal port to freeze before adoption

The eventual port should expose operations, not upstream implementation types:

```rust
trait ManagedAgentPort {
    async fn submit(&self, request: SubmitManagedTask) -> Result<TaskAcceptance, PortError>;
    async fn task(&self, task_id: &str) -> Result<ManagedTaskView, PortError>;
    async fn cancel(&self, task_id: &str, reason: Option<&str>) -> Result<CancellationView, PortError>;
    async fn graph(&self, root_operation_id: &str) -> Result<ManagedGraphView, PortError>;
    async fn snapshot(&self, request: ReconciliationRequest) -> Result<ReconciliationSnapshot, PortError>;
}
```

This is a shape constraint, not code to add before upstream DTOs stabilize. Port projections must retain unknown extension data or explicit unsupported-extension evidence rather than silently dropping it.

## Mapping from current Auspex model

| Current Auspex concept | Future A2A mapping | Migration rule |
|---|---|---|
| `ManagedRunId` | local correlation ID, not A2A task ID | retain as adapter-local identity during migration |
| `OmegonTaskId` | execution-provider ID | preserve as provider evidence/extension, never substitute for A2A task ID |
| `WorkerId` | logical agent plus runtime incarnation | split when Styrene identity/runtime IDs are available |
| parent session/turn | A2A context plus causal parent task | do not infer parent task from transcript position |
| `ManagedRunState` | projection of A2A task state plus delivery/cancellation evidence | keep transport receipt state separate from task outcome |
| supervisor events | A2A task/status/artifact events plus provider evidence | preserve ordering and sequence-gap evidence |
| result string | A2A message/artifact/reference | large results become references; no unbounded conversion |
| instance route | Auspex transport ownership | remains outside signed A2A payload |

## Upstream completion gate

Do not add a `styrene-a2a` or `styrene-ipc` dependency until all mandatory gates pass against one immutable upstream commit.

### Mandatory artifacts

- [ ] `styrene-a2a` envelope profile v1 has frozen field indices and golden vectors.
- [ ] Official SDK round-trip fixtures exist for message, task, status event, artifact event, and Agent Card.
- [ ] Delegation extension validates root preservation, one-parent ancestry, cycle rejection, depth attenuation, and authority attenuation.
- [ ] Typed receipt, protocol error, graph, and reconciliation snapshot DTOs exist.
- [ ] Sequence scope, duplicate/conflict behavior, gap handling, and runtime restart behavior are tested.
- [ ] `styrene-services::agent` has durable repository semantics and idempotency tests.
- [ ] `DaemonAgents` exposes submit/task/cancel/graph/snapshot with stable projection DTOs.
- [ ] Recognized invalid A2A traffic cannot fall through to chat.
- [ ] Mixed-version IPC fixtures pass without changing existing opcode bytes.
- [ ] Upstream publishes the exact commit and validation command used for handoff.

### Security gates

- [ ] Agent identity resolves through `styrene-identity`; self-assertion is insufficient.
- [ ] Signing input and protected fields are frozen and covered by vectors.
- [ ] Grant validation binds signer, target, expiry, root operation, and parent grant.
- [ ] Child authority can only attenuate.
- [ ] Replay retention is at least the maximum envelope lifetime plus adapter retry window.
- [ ] Unknown required extensions reject before execution.
- [ ] Transport routing metadata is excluded from signed domain payloads.

## Handoff bundle requested from the upstream agent

At completion, request one bundle containing:

1. upstream commit SHA;
2. `cargo test`/validation transcript;
3. public Rust API inventory for `styrene-a2a` and `DaemonAgents`;
4. golden JSON/CBOR vectors and schema/profile versions;
5. compatibility matrix for old/new Rust and Python IPC peers;
6. known deferrals and unsupported extension behavior;
7. migration or persistence schema notes;
8. one end-to-end nested delegation trace including snapshot reconciliation and partial cancellation.

No integration should be based only on prose stating completion.

## Auspex adoption sequence

1. **Vendor compatibility fixtures only.** Copy immutable upstream vectors into `auspex-core/tests/fixtures/styrene_a2a/<profile-version>/` with source SHA and license metadata.
2. **Add a feature-gated dependency.** Use a released version or immutable git revision; never a mutable sibling path in committed manifests.
3. **Implement pure conversions.** Add `styrene_a2a_adapter` with conversion tests and no controller or UI changes.
4. **Introduce `ManagedAgentPort`.** Put the current Omegon runtime behind one implementation; add the Styrene implementation alongside it.
5. **Run dual-read shadow mode.** Existing Omegon projection remains authoritative while Styrene projection is computed and compared. No duplicate dispatch.
6. **Reconcile divergence.** Compare identity, parent/root links, state, cancellation evidence, sequence watermarks, and artifacts.
7. **Switch authority explicitly.** Promote Styrene projection only after shadow fixtures and live traces agree.
8. **Remove duplicate domain types only after migration.** Keep provider evidence and transport state where A2A intentionally has no equivalent.

## Compatibility test matrix for Auspex

| Scenario | Required assertion |
|---|---|
| Fresh root task | one local run maps to one A2A task/root operation |
| Nested delegate | parent/root/runtime identities survive round trip |
| Duplicate envelope | one effect; duplicate evidence retained |
| Sequence gap | projection becomes incomplete and requests snapshot |
| Runtime restart | logical agent stable, runtime ID changes, reconciliation required |
| Cancellation | requested, accepted, and termination-confirmed remain independent |
| Partial descendant loss | ancestor is not reported fully terminated |
| Unknown optional extension | retained/ignored per A2A rules |
| Unknown required extension | rejected before dispatch |
| Oversized artifact | represented by resource/content reference |
| Old daemon | capability absence hides/disables A2A path without breaking Omegon control |
| Corrupt signature/grant escalation | rejected with safe typed error |

## Operational watch procedure while upstream is in progress

Use read-only checks only:

```bash
cd ../styrene-rs
git log -1 --format='%H %s'
git status --short crates/libs/styrene-a2a crates/libs/styrene-services crates/libs/styrene-ipc docs/a2a-integration-*
cargo test -p styrene-a2a
```

Do not edit, format, rebase, or commit the upstream agent's files. A dirty working tree is expected while it works. Reassess only when it provides a commit SHA or asks for contract review.

## Current Auspex freeze

Until handoff:

- no Dioxus/UI work based on speculative A2A fields;
- no sibling path dependency on `../styrene-rs`;
- no new A2A task or graph DTOs in Auspex;
- no opcode assumptions;
- no replacement of the working Omegon supervisor path;
- backend fixes unrelated to A2A may continue if they preserve the adapter boundary.
