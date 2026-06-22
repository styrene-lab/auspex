---
title: Auspex agent attach, launch, and profile creation affordance paths
status: exploring
tags: [auspex, agents, ui, profiles, lifecycle]
parent: auspex-multi-agent-runtime
---

# Auspex agent attach, launch, and profile creation affordance paths

## Overview

Auspex needs an operator path between defined agent-console states, not separate pages for each lifecycle action. The active Agents surface should remain the canonical place where an operator moves from no/single agent operation into a small multi-agent roster and eventually larger fleet grouping.

This node lays down the design elements for three affordances:

1. **Attach existing runtime** — bind Auspex to an already-running Omegon instance.
2. **Launch from profile** — start a new runtime instance from an existing profile definition.
3. **Create profile** — author a new Omegon-compatible profile through the GUI, then optionally launch it.

Ownership boundary:

- `omegon-secundus` owns profile schema, profile semantics, runtime control methods, and local profile lifecycle.
- `omegon-armory` owns upstream/self-hosted registry and distribution concepts for profiles/agents.
- Auspex owns the localized GUI inventory/cache, operator affordances, instance selection, and launch/attach workflow presentation.

## State path

```text
No managed agents
  -> attach existing runtime OR launch/create first profile
Single agent console
  -> add agent
Small roster (2-5 agents)
  -> select/switch/coordinate agents
Grouped fleet (6+ agents)
  -> group by role/project/runtime/health and search/attention queue
```

The roster should appear as a consequence of state:

- 0 agents: empty launch state.
- 1 agent: focused console, no roster chrome.
- 2-5 agents: compact roster + selected console.
- 6+ agents: grouped selector / attention queue.

## Affordance placement

The active Agents surface should gain an **Add agent** action in the selected agent live panel. It should not become a new top-level nav item.

Recommended placement:

```text
Agent live panel
├─ identity/runtime/telemetry board
├─ [Add agent] action
└─ warning/availability badges
```

Opening **Add agent** should reveal an inline drawer or modal stepper with three source choices:

```text
Add agent
├─ Attach existing runtime
├─ Launch from profile
└─ Create profile
```

## Attach existing runtime

### Operator intent

Bind Auspex to a runtime that already exists without changing its profile or starting a new process.

### Required inputs

- Control-plane URL or discovered runtime selection.
- Auth/token reference if required.
- Expected role/profile label, if known.

### Preflight

- Reachability: `/api/state`, `/api/readyz`, and capability surfaces.
- Instance descriptor schema compatibility.
- Role/profile compatibility with the intended workspace.
- Authorization scope for command/control.

### Success transition

```text
single agent console -> compact roster + selected attached runtime
```

If this is the first agent:

```text
empty state -> selected attached runtime console
```

### Failure states

- Endpoint unreachable.
- Descriptor schema incompatible.
- Auth missing/invalid.
- Runtime is already registered under another identity.
- Runtime role/profile incompatible with requested use.

## Launch from profile

### Operator intent

Start a new Omegon runtime instance using an existing profile definition.

### Required inputs

- Profile id/path from localized registry.
- Model/provider selection filtered by authenticated providers.
- Runtime substrate:
  - native/local
  - host-shim OCI
  - Kubernetes/orchestrated
- Workspace/project binding.

### Preflight

- Profile exists and validates against `omegon-secundus` schema.
- Required provider is authenticated.
- Required secrets are present or grantable.
- Selected substrate is available and permitted.
- Workspace path/project binding is valid.
- Port/endpoint allocation is available for local launch.

### Success transition

```text
selected console -> compact roster + new launched agent selected
```

### Failure states

- Profile validation failure.
- Provider/model unavailable.
- Required secret missing.
- Runtime substrate unavailable.
- Launch failed before startup descriptor.
- Startup descriptor emitted but readiness failed.

## Create profile

### Operator intent

Author an Omegon-compatible profile through Auspex without needing to use the TUI profile workflow.

### Ownership constraint

Auspex must not redefine profile semantics. The form must be generated from, validated against, or directly mapped to `omegon-secundus` profile schema and lifecycle operations.

### Required inputs

Minimum viable GUI profile fields:

- Profile display name.
- Profile id/path.
- Role/posture.
- Preferred model/provider from authenticated provider matrix.
- Capability set / tools, as schema-owned profile fields.
- Secret requirements / grants.
- Workspace/project scope.

### Preflight

- Profile id uniqueness in localized registry.
- Schema validation through `omegon-secundus` semantics.
- Required provider availability.
- Required secret/grant availability.
- Optional registry publish eligibility, but publishing is not required for local creation.

### Success transition

After save:

```text
create profile -> launch from profile prefilled
```

The operator can then choose:

- Save only.
- Save and launch.
- Save and publish later, once armory publishing is implemented.

## Localized registry model

Auspex should present a localized registry/cache that merges:

- profiles known to `omegon-secundus`
- profiles shipped with the project
- profiles discovered from `omegon-armory` or self-hosted registry sources
- local drafts created in Auspex
- runtime instances currently attached/launched

Important distinction:

```text
profile definition != runtime instance != deployment environment
```

The UI must keep those separated:

- Profile definition: durable recipe/identity.
- Runtime instance: live process/container/pod with descriptor and telemetry.
- Deployment environment: local/OCI/Kubernetes substrate and policy.

## View model sketch

```rust
struct AgentConsoleModel {
    agents: Vec<AgentConsoleEntity>,
    selected_id: Option<String>,
    add_agent_open: bool,
    add_agent_mode: Option<AddAgentMode>,
}

enum AddAgentMode {
    AttachExistingRuntime,
    LaunchFromProfile,
    CreateProfile,
}

struct AgentConsoleEntity {
    id: String,
    label: String,
    role: String,
    profile_id: Option<String>,
    profile_path: Option<String>,
    model: Option<String>,
    runtime_label: String,
    workspace_label: String,
    capability_tier: Option<String>,
    telemetry: AgentTelemetryModel,
    chat_available: bool,
}

struct AgentTelemetryModel {
    turns: u32,
    tool_calls: u32,
    context_tokens: Option<u64>,
    context_window: Option<u64>,
}
```

## UX rules

- Do not add top-level nav for attach/launch/create.
- Do not show roster chrome for one agent.
- Add agent action opens a contained workflow, then returns to selected console.
- Runtime controls apply to the selected live instance only.
- Profile creation edits durable profile definitions, not live runtime unless explicitly followed by launch/apply.
- Provider/model choices must be filtered by authenticated provider availability.
- Capability tier should remain a classification label unless `omegon-secundus` exposes a real mutable control.

## Open questions

- [assumption] Auspex can ask `omegon-secundus` for profile schema/validation instead of vendoring or reimplementing profile semantics.
- [assumption] `omegon-armory` will expose enough registry metadata for Auspex to display remote/self-hosted profile sources without owning distribution logic.
- What is the canonical attach API: direct URL entry, discovery list, or both?
- What is the canonical launch API for non-primary agents: existing Omegon command method, Auspex host action, or new control-plane endpoint?
- Where should local profile drafts live before they are saved into `omegon-secundus` profile storage?
- Which profile fields are required for a minimal GUI-created profile versus advanced/power-user fields that should stay collapsed?
- How should credentials/secrets be granted to a launched secondary agent without over-broad inheritance from the primary runtime?
- When does a runtime become part of the localized registry: after descriptor fetch, after readiness, or after explicit operator acceptance?

## First implementation slice

1. Add `selected_agent_id` and `add_agent_open` to the Agents surface state.
2. Introduce `AgentConsoleEntity` and build the current primary runtime through it.
3. Add an `Add agent` button to the live panel.
4. Add a drawer with the three source choices and preflight placeholders.
5. Keep attach/launch/create actions dry-run until concrete control-plane endpoints are verified.
