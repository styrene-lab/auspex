---
id: auspex-omegon-control-surface-catalog
title: "Omegon canonical control-surface catalog"
status: exploring
parent: auspex-omegon-runtime-control-plane
tags: [runtime-control]
open_questions:
  - "Which registry is authoritative when command metadata, backend metadata, and runtime behavior disagree?"
  - "How will capability negotiation report operations unavailable on an attached Omegon version?"
  - "[assumption] The catalog can be versioned independently while retaining stable canonical action IDs."
dependencies: []
related:
  - auspex-omegon-runtime-control-plane
  - auspex-worker-profiles
---

# Omegon canonical control-surface catalog

## Overview

Inventory and normalize every Omegon operator control across CLI, slash, TUI menus, ACP, IPC, WebSocket, HTTP, model tools, and extension RPC. Define canonical domains, resource kinds, mutability, required authority, persistence, runtime effect, side effects, and preferred/fallback transports.

## Research

### Omegon 0.29 already has the canonical semantic boundary

Verified against pinned release source `release/0.29` at `547b4609`:

- `core/crates/omegon/src/operator_commands.rs` defines `InterfaceControlRequest` as the surface-neutral operator boundary.
- It already covers profile, permission trust, runtime inventory, workspace, skills, extensions, Armory, catalog, plugins, secrets, variables, model, thinking, session, and lifecycle operations.
- `core/crates/omegon/src/ui_runtime/client_api.rs` defines versioned external `ClientEnvelope` DTOs and a deliberately smaller `ClientControlRequestDto` V1 subset.
- WebSocket ingress sends semantic requests through `WebCommand::ExecuteControl`; slash execution is therefore not required as the canonical mechanism.

This establishes the authority order for the catalog:

1. `InterfaceControlRequest` names canonical runtime operations.
2. `ClientControlRequestDto` defines which operations are stable and externally exposed for a protocol version.
3. Transport adapters bind DTOs to ACP, IPC, WebSocket, HTTP, or local channels.
4. Slash commands are compatibility aliases and discovery affordances, not the source of truth.

### Initial domain inventory

| Domain | Existing semantic requests | Initial exposure |
|---|---|---|
| Runtime | `RuntimeInventoryStatus`, `RuntimeSubstrateRefresh`, `StatusView` | Read now; refresh operator action |
| Profiles | `ProfileView`, `ProfileCapture`, `ProfileApply`, `ProfileUse`, profile extension/persona/tone/MQTT edits | Read first; revisioned mutation later |
| Workspace | status/list/new/destroy/adopt/release/archive/prune/bind/role/kind | Read first; trust and destructive mutations approval-gated |
| Skills | view/help/install/get/delete | Inventory read; install/delete privileged |
| Extensions | view/get/search/init/install/remove/update/enable/disable | Inventory read; executable mutations privileged |
| Packages | Armory, catalog, and plugin browse/install/remove/update | Browse read; install/update/remove privileged |
| Permissions | view, trust add/remove | Read first; trust changes explicit operator approval |
| Models | view/list/set/provider/policy/clear override | Read and session mutation; persistence explicit |
| Secrets | view/set/get/delete and vault operations | Metadata-only reads; values never projected into menus or launch specs |

## Decisions

### Omegon's InterfaceControlRequest is the canonical operation registry

**Status:** accepted

Auspex will not derive runtime administration from slash-command names. Stable external DTOs will project selected `InterfaceControlRequest` operations with explicit versioning.

### External exposure is an allowlisted versioned subset

**Status:** accepted

The full internal request enum is not automatically remotely callable. Each protocol version exposes a reviewed subset with authority, persistence, confirmation, and side-effect metadata.

### Read-only inventory is the first implementation tranche

**Status:** accepted

The initial slice exposes runtime, profile, workspace, skill, extension, package, permission, and model inventory. Mutations follow after revision and approval contracts are decided.

## Open Questions

The structured questions in frontmatter are the current frontier. Before deciding this node, explicitly ask: **What assumptions is this design making that have not been stated?** Record each answer as an `[assumption]` question until validated.

### Initial read-only action set

| `action_id` | Omegon semantic request | Authority | Persistence | Effect | Preferred binding |
|---|---|---|---|---|---|
| `runtime.inventory.read` | `RuntimeInventoryStatus` | observer | none | none | client API V2 |
| `profile.inventory.read` | `ProfileView` | observer | none | none | client API V2 |
| `workspace.status.read` | `WorkspaceStatusView` | observer | none | none | client API V2 |
| `workspace.inventory.read` | `WorkspaceList` | observer | none | none | client API V2 |
| `skill.inventory.read` | `SkillsView` | observer | none | none | client API V2 |
| `extension.inventory.read` | `ExtensionView` | observer | none | none | client API V2 |
| `package.armory.read` | `ArmoryView` | observer | none | none | client API V2 |
| `package.catalog.read` | `CatalogView` | observer | none | none | client API V2 |
| `plugin.inventory.read` | `PluginsView` | observer | none | none | client API V2 |
| `permission.policy.read` | `PermissionsView` | observer | none | none | client API V2 |
| `model.effective.read` | `ModelView` | observer | none | none | client API V1 |
| `model.catalog.read` | `ModelList` | observer | none | none | client API V1 |

`client API V2` is a required follow-up in Omegon 0.29: the internal semantic requests exist, but the current external V1 DTO only exposes a smaller subset including model operations. Until V2 exists, local IPC or local control channels may bind the same action IDs; slash execution remains fallback-only.

## Canonical action descriptor

Every operation exposed by Auspex must project this metadata independently of renderer or transport:

| Field | Meaning |
|---|---|
| `action_id` | Stable namespaced identifier, e.g. `runtime.inventory.read` |
| `resource_kind` | Runtime, profile, workspace, skill, extension, package, permission, model, secret metadata |
| `mode` | Read, create, update, delete, execute, lifecycle |
| `authority` | Observer, operator, administrator |
| `persistence` | None, session, user profile, project profile, workspace registry, installation |
| `runtime_effect` | None, live, reload, restart, next launch |
| `confirmation` | None, review, explicit confirm, external approval |
| `transport` | Preferred typed binding and compatibility fallbacks |
| `capability` | Runtime-advertised support identifier and minimum protocol version |
| `side_effects` | Files, processes, network, credentials, trust, package installation |

The menu projection consumes this descriptor plus live capability and authority state. It does not infer safety from labels.
