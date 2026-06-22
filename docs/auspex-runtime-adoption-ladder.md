+++
title = "Auspex Runtime Adoption: Detected, Attached, Managed"
tags = ["auspex", "omegon", "runtime", "adoption", "authority"]
+++

+++
id = "auspex-runtime-adoption-ladder"
kind = "design_node"

[data]
title = "Auspex Runtime Adoption: Detected, Attached, Managed"
status = "exploring"
issue_type = "architecture"
priority = 1
parent = "0295515b-4c51-4e93-a775-1eed5cd71003"
dependencies = ["runtime-observation-authority-invariant", "native-local-discovery-ownership-enrichment", "auspex-instance-lifecycle-policy"]
open_questions = [
  "[assumption] Existing Omegon runtimes expose enough descriptor identity to distinguish owner, workspace, role, and command capabilities.",
  "[assumption] Operators need to attach foreign runtimes for command without granting lifecycle authority.",
  "Should adoption receipts live in the instance registry, a separate adoption ledger, or both?",
  "What exact runtime descriptor fields prove ownership transfer eligibility?",
  "How should Auspex represent command authorization when a runtime supports prompt submission but not runtime control methods?"
]
+++

# Auspex Runtime Adoption: Detected, Attached, Managed

Parent: [[runtime-observation-authority-invariant]]  
Related: [[native-local-discovery-ownership-enrichment]], [[auspex-instance-lifecycle-policy]], [[local-attach-persistence-and-rehydration]], [[auspex-instance-registry-schema]]

## Terminology decision

Use three operator-facing terms:

```text
Detected  -> Auspex can see/probe the runtime.
Attached  -> Auspex can command the runtime, but does not own its lifecycle.
Managed   -> Auspex has durable management responsibility for the runtime.
```

Avoid exposing `observed`, `adopted`, or `owned` as primary UI labels. Those are implementation details or sub-states. The operator needs to know what Auspex can safely do.

## Why this matters

A runtime listening on a known port is not automatically Auspex-owned. The previous failure mode was that Auspex treated a non-Auspex runtime on `7842` as something to reap. The adoption model must make that impossible by construction.

The runtime relationship must separate:

- detection / observation
- command authority
- lifecycle authority
- profile/config authority
- durable registry responsibility
- true ownership

## Operator-facing states

| State | Operator meaning | Auspex can read? | Auspex can chat? | Auspex can mutate runtime controls? | Auspex can restart/stop? | Auspex can rewrite profile? |
|---|---|---:|---:|---:|---:|---:|
| `Detected` | Runtime exists and passed some probe. | yes, limited by probe | no | no | no | no |
| `Attached` | Operator authorized this runtime as a command target. | yes | yes | only if advertised/authorized | no | no |
| `Managed` | Auspex is responsible for durable supervision. | yes | yes | yes, if supported | maybe, by management mode | maybe, by management mode |

## Internal sub-states

Operator-facing states map to more precise internal state.

```rust
enum RuntimeRelationship {
    Detected(DetectedRuntime),
    Attached(AttachedRuntime),
    Managed(ManagedRuntime),
}

enum DetectionFreshness {
    Candidate,
    ProbedFresh,
    PersistedStale,
    Lost,
}

enum CommandAuthority {
    None,
    PromptOnly,
    PromptAndCancel,
    RuntimeControls,
    Admin,
}

enum LifecycleAuthority {
    None,
    Delegated,
    SupervisorOwned,
}

enum ProfileAuthority {
    None,
    RuntimeDelegated,
    RegistryOwned,
}

enum RuntimeOwnership {
    AuspexOwned,
    OperatorOwned,
    External,
    Unknown,
}
```

`Managed` does not always mean `AuspexOwned`. A systemd service or Kubernetes deployment can be managed externally while Auspex has durable monitoring and delegated control.

## State 1 — Detected

### Definition

Auspex found a runtime candidate by process table, known port, registry hint, manually entered URL, container inventory, or remote discovery.

### Evidence

Any of:

- process exists
- port is listening
- `/api/readyz` responds
- `/api/state` responds
- IPC socket exists
- registry record exists but freshness is unknown

### Allowed actions

- probe health/readiness
- read descriptor/state if exposed
- assess compatibility
- display endpoint/process/container identity
- show attach/manage options

### Forbidden actions

- prompt submission
- runtime control mutation
- stop/restart/reap
- profile/config write
- ownership upgrade

### UI

Label:

```text
Detected runtime
```

Actions:

```text
Refresh probe
Attach...
Manage...
Ignore
```

Primary warning:

```text
Detected does not grant command or lifecycle authority.
```

## State 2 — Attached

### Definition

The operator explicitly authorizes Auspex to use the runtime as a command target. The runtime remains externally/operator owned unless a separate management action occurs.

### Evidence

Required:

- compatible runtime descriptor or control-plane state
- command transport validation
- operator confirmation
- token/credential reference if needed
- adoption receipt with lifecycle authority set to `None`

### Allowed actions

- prompt submission
- turn cancel if supported
- runtime controls if explicitly advertised and authorized
- display requested vs observed runtime state
- persist command binding
- detach command binding

### Forbidden actions

- process kill
- port reaping
- profile rewrite
- service restart
- claiming Auspex ownership

### UI

Label:

```text
Attached runtime
```

Actions:

```text
Chat
Sync state
Detach
Manage...
```

Primary warning:

```text
Attached means command authority, not lifecycle ownership.
```

### Runtime controls

Runtime controls are part of the chat/ACP next-turn envelope, not a saved form. The UI must distinguish:

```text
requested: model/thinking/context chosen by operator
observed: latest runtime state reported by the runtime
sync: matched | pending | mismatch | not reported
```

## State 3 — Managed

### Definition

Auspex has durable responsibility for supervising the runtime. Management may be delegated/external or Auspex-owned.

### Managed modes

```rust
enum ManagedMode {
    ExternalMonitored,
    DelegatedLifecycle,
    AuspexOwned,
}
```

### 3a. Managed external monitored

Use for runtimes that Auspex should remember and monitor but not restart directly.

Examples:

- operator-started local daemon
- launchd/systemd service
- remote control-plane endpoint
- Kubernetes service managed by another controller

Allowed:

- durable registry record
- freshness/stale/lost state transitions
- health monitoring
- command authority if also attached

Forbidden:

- local PID kill
- profile rewrite
- restart unless delegated by runtime/service API

### 3b. Managed delegated lifecycle

Use when the runtime exposes a lifecycle API or is controlled through a known substrate.

Examples:

- container backend exposes restart/remove for a known container id
- Kubernetes deployment with permitted rollout/restart action
- service manager grants restart permission

Allowed:

- restart/stop through delegated API
- status reconciliation
- detach management without killing runtime where possible

Required:

- delegated lifecycle capability
- policy gate
- adoption receipt with `lifecycle_authority = Delegated`

### 3c. Managed Auspex-owned

Use only when Auspex launched the runtime or the runtime explicitly transferred ownership.

Allowed:

- full command/control
- restart/stop/reap
- profile/config write within receipt scope
- port cleanup within receipt scope

Required:

- Auspex launch receipt or takeover receipt
- matching instance id
- current descriptor schema compatibility
- fresh readiness
- runtime acknowledgement for takeover, if not originally launched by Auspex

## Adoption receipts

Every transition beyond `Detected` should produce or update an adoption receipt.

```json
{
  "schema_version": 1,
  "instance_id": "omg_...",
  "endpoint": "http://127.0.0.1:7842",
  "relationship": "attached",
  "managed_mode": null,
  "freshness": "probed_fresh",
  "ownership": "external",
  "command_authority": "prompt_and_cancel",
  "lifecycle_authority": "none",
  "profile_authority": "none",
  "workspace": "/path/to/project",
  "control_plane_schema": 1,
  "capabilities": [
    "state.read",
    "prompt.submit",
    "turn.cancel",
    "runtime.set_model"
  ],
  "token_ref": "auspex/local/omg_...",
  "operator_confirmed": true,
  "runtime_acknowledged": false,
  "adopted_at": "2026-06-22T00:00:00Z",
  "last_verified_at": "2026-06-22T00:00:10Z"
}
```

Receipts must distinguish operator confirmation from runtime acknowledgement.

## Transition rules

### Detected → Attached

Requires:

- fresh compatible probe
- command transport test
- operator confirmation
- credential/token reference, if required

Does not grant:

- lifecycle authority
- profile authority
- ownership

### Detected → Managed external

Requires:

- fresh compatible probe or accepted stale registry record
- operator confirmation to persist
- management mode selection

Does not automatically grant:

- command authority
- restart authority

### Attached → Managed external

Requires:

- existing attach receipt
- operator chooses durable monitoring/management
- management capabilities are recorded separately

### Managed external → Managed delegated lifecycle

Requires:

- lifecycle capability proof
- policy gate
- operator confirmation

### Any → Managed Auspex-owned

Requires one of:

- Auspex launch receipt
- explicit runtime ownership-transfer acknowledgement

Operator intent alone is insufficient.

## Port policy

Local control-plane ports must not imply ownership.

```text
7842          conventional/default external or primary Omegon endpoint
7843..7899    embedded Auspex fallback ports
7900..7999    locally deployed auxiliary agents
container     internal 7842, mapped externally by OCI/k8s runtime
```

Rules:

- Auspex must not reap arbitrary listeners on `7842`.
- Embedded primary uses `7842` only if free; otherwise it chooses a fallback port.
- Locally deployed agents allocate monotonically from `7900..7999`.
- OCI agents can listen on container-internal `7842`; external mappings are owned by the container backend.

## UI layout implications

### Runtime inventory

Group by relationship:

```text
Detected
Attached
Managed
```

Do not group first by ownership, because the operator's immediate question is “what can Auspex do with this?”

### Candidate card

Each runtime card should show:

```text
relationship: Detected | Attached | Managed
freshness: fresh | stale | lost
command: none | prompt | controls | admin
lifecycle: none | delegated | owned
ownership: unknown | external | operator | auspex
workspace: reported path or unknown
endpoint: URL/socket/container
```

### Actions by state

Detected:

```text
Probe
Attach
Manage
Ignore
```

Attached:

```text
Chat
Sync
Detach
Manage
```

Managed:

```text
Open
Sync
Detach command
Unmanage
Restart/Stop only if lifecycle authority exists
```

## Decisions

### D1 — Use Detected, Attached, Managed as operator-facing labels

Status: proposed

These labels map to operator expectations better than discovered/observed/adopted/owned.

### D2 — Attached means command authority only

Status: proposed

Attached runtimes are not lifecycle-owned. Auspex must not kill, restart, or rewrite them unless separately managed.

### D3 — Managed has modes

Status: proposed

Managed must split into external monitored, delegated lifecycle, and Auspex-owned. Otherwise “managed” will become another overloaded word.

### D4 — Full ownership requires runtime acknowledgement or Auspex launch receipt

Status: proposed

Operator intent can authorize management, but it cannot prove process ownership or safe cleanup scope by itself.

### D5 — Port allocation is not authority evidence

Status: proposed

A runtime on a known/default port is only Detected until descriptor and receipt evidence prove more.

## Implementation slices

1. [[auspex-runtime-relationship-registry]] — add relationship, authority axes, and receipts.
2. [[auspex-runtime-relationship-ui]] — inventory grouped by Detected / Attached / Managed.
3. [[auspex-runtime-attach-flow]] — attach wizard and command transport validation.
4. [[auspex-runtime-managed-flow]] — external monitored/delegated/Auspex-owned management modes.
5. [[auspex-local-agent-port-policy]] — monotonic `7900..7999` local deployed-agent allocation.

## Open questions

- [assumption] Existing Omegon runtimes expose enough descriptor identity to distinguish owner, workspace, role, and command capabilities.
- [assumption] Operators need to attach foreign runtimes for command without granting lifecycle authority.
- Should adoption receipts live in the instance registry, a separate adoption ledger, or both?
- What exact runtime descriptor fields prove ownership transfer eligibility?
- How should Auspex represent command authorization when a runtime supports prompt submission but not runtime control methods?

## Integration check against `../omegon-secundus` — 2026-06-22

Recent inspection of the sibling Omegon checkout confirms the adoption design aligns with current exposed contracts, with several important integration details.

### Confirmed control-plane surfaces

Omegon currently exposes these relevant runtime/control surfaces:

- HTTP startup/state/ready URLs:
  - `/api/startup`
  - `/api/state`
  - `/api/readyz`
- WebSocket command handling for:
  - `set_model`
  - `set_thinking`
  - `set_context_class`
  - `set_max_turns`
- IPC request handling for:
  - `get_state`
  - `submit_prompt`
  - `cancel`
  - `run_slash_command`
  - `set_model`
  - `set_thinking`
  - `set_context_class`
  - `set_max_turns`
- ACP/read-only surfaces:
  - `_runtime/status`
  - `_provider/status`
  - `_runtime/capabilities`
  - `_capabilities/inventory`

### Confirmed descriptor fields

`omegon-traits` defines `IpcStateSnapshot` with a required `instance: OmegonInstanceDescriptor`. The descriptor already carries the fields Auspex needs for Detected / Attached / Managed classification:

```rust
pub struct OmegonInstanceDescriptor {
    pub schema_version: u16,
    pub identity: OmegonIdentity,
    pub ownership: OmegonOwnership,
    pub placement: OmegonPlacement,
    pub control_plane: OmegonControlPlane,
    pub runtime: OmegonRuntime,
}
```

Important subfields:

```rust
OmegonIdentity {
    instance_id,
    workspace_id,
    session_id,
    role,
    profile,
}

OmegonOwnership {
    owner_kind,
    owner_id,
    parent_instance_id,
}

OmegonPlacement {
    kind,
    host,
    pid,
    cwd,
    namespace,
    pod_name,
    container_name,
}

OmegonControlPlane {
    server_instance_id,
    protocol_version,
    schema_version,
    omegon_version,
    capabilities,
    ipc_socket_path,
    http_base,
    startup_url,
    state_url,
    ws_url,
    auth_mode,
    auth_source,
    http_transport_security,
    ws_transport_security,
}

OmegonRuntime {
    deployment_kind,
    runtime_mode,
    runtime_profile,
    autonomy_mode,
    health,
    provider_ok,
    memory_ok,
    cleave_available,
    queued_events,
    transport_warnings,
    runtime_dir,
    context_class,
    thinking_level,
    capability_tier,
    execution_substrate,
}
```

These fields are sufficient for the first registry receipt shape. Auspex should not invent ownership from ports or process names; it should derive relationship candidates from descriptor identity, ownership, placement, control-plane capabilities, and runtime health.

### Capability mapping

`omegon-traits::IpcCapability` currently includes:

```text
state.snapshot
prompt.submit
turn.cancel
model.view
model.list
model.set
thinking.set
dispatcher.switch
slash_commands
shutdown
```

Initial Auspex mapping:

| Omegon evidence | Auspex relationship implication |
|---|---|
| state/ready responds, descriptor valid | `Detected` |
| `prompt.submit` available and operator confirms | `Attached(command=PromptOnly)` |
| `turn.cancel` also available | `Attached(command=PromptAndCancel)` |
| `model.set` / `thinking.set` plus web/IPC set methods | `Attached(command=RuntimeControls)` |
| `shutdown` plus ownership/receipt evidence | lifecycle candidate, not automatic |
| `owner_kind = Auspex` and matching launch/adoption receipt | `Managed(AuspexOwned)` |
| `owner_kind = Systemd/Kubernetes/Operator` | `ManagedExternal` or `Attached`, not owned |

### Drift from earlier design

The current Omegon control model distinguishes runtime route state more deeply than our first draft did:

- `set_model` remains available, but Omegon also has `set_model_intent`, `set_model_provider`, and `set_model_policy` paths internally.
- Changelog notes indicate startup fallback behavior is explicit and profile-preserving; Auspex must therefore avoid assuming the selected/requested model is the served bridge model.
- Omegon already has ACP `_runtime/status` and `_provider/status`; Auspex should consume these for observed/runtime reconciliation when available rather than relying only on session provider summaries.

### Design adjustment

For runtime controls, the receipt/UI should distinguish:

```text
requested envelope  -> what Auspex asked for next turn
runtime route state -> what Omegon accepted as selected policy/intent/provider/model
served bridge       -> what model/provider actually answers requests
```

This means the `Attached` UI needs more than `requested vs observed`. It should eventually show:

```text
requested: openai-codex:gpt-5.5
selected:  openai-codex:gpt-5.5
served:    anthropic:claude-sonnet-4-6 fallback   # if fallback is active
status:    mismatch / fallback / in-sync
```

### Integration decisions

1. Use `IpcStateSnapshot.instance` as the strongest local adoption evidence when IPC is available.
2. Use HTTP `/api/state` descriptor as the next-best Detected/Attached evidence.
3. Use ACP `_runtime/status`, `_provider/status`, and `_runtime/capabilities` for runtime truth where available.
4. Treat WebSocket command support as a command transport, not ownership proof.
5. Treat `shutdown` capability as dangerous: it is lifecycle capability only when paired with `Managed` authority and receipt evidence.
6. Treat `owner_kind`, `placement`, `execution_substrate`, and launch/adoption receipts as the authority source for Managed modes.

### Follow-up implementation impact

- `auspex-runtime-relationship-registry` should store descriptor excerpts: `identity`, `ownership`, `placement`, `control_plane.capabilities`, `runtime.execution_substrate`.
- `auspex-runtime-attach-flow` should probe capabilities before enabling chat/runtime controls.
- `auspex-runtime-managed-flow` should require ownership or delegated lifecycle proof before exposing stop/restart/reap.
- `auspex-local-agent-port-policy` remains valid: ports are transport coordinates only, never authority.
