---
id: auspex-instance-registry-schema
title: "Auspex instance registry schema"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions: []
dependencies: []
related:
  - auspex-multi-agent-runtime
---

# Auspex instance registry schema

## Overview

Define the persistent schema for tracking logical Omegon workers across local, detached, OCI, and Kubernetes backends, including identity, placement, control-plane connectivity, lifecycle, and ownership.

## Decisions

### Registry records split desired state from observed state

**Status:** accepted

The registry must distinguish what Auspex asked for from what currently exists. This is mandatory for OCI and Kubernetes workers, where placement and readiness are asynchronous.

### Tokens are referenced, not embedded

**Status:** accepted

Worker auth material should be stored via a secret reference where possible (`token_ref`) rather than inline in the registry record.

### Dispatcher attachment requires logical identity plus authenticated control-plane binding

**Status:** proposed

A dispatcher or other reattachable worker should not be trusted by `instance_id` alone. The registry should support bindings that combine logical worker identity with control-plane verification material such as `token_ref`, expected schema/version, and last verified endpoint details.

## Canonical on-disk record shape

Use one file per worker instance in a registry directory such as:

```text
~/.config/auspex/instances/<instance-id>.json
```

### JSON record shape

```json
{
  "schema_version": 1,
  "identity": {
    "instance_id": "omg_01HVK6K4QFQF8B2W2J7Q6M7Y3S",
    "role": "supervised-child",
    "profile": "cheap-subtask",
    "status": "busy",
    "created_at": "2026-04-03T12:00:00Z",
    "updated_at": "2026-04-03T12:03:42Z"
  },
  "ownership": {
    "owner_kind": "auspex-session",
    "owner_id": "session_01HV...",
    "parent_instance_id": "omg_primary_01HV..."
  },
  "desired": {
    "backend": {
      "kind": "kubernetes",
      "image": "ghcr.io/org/omegon:v0.15.7",
      "namespace": "auspex",
      "resources": {
        "cpu": "500m",
        "memory": "1Gi"
      }
    },
    "workspace": {
      "cwd": "/repo/path",
      "workspace_id": "repo:8f2f4c1",
      "branch": "main"
    },
    "task": {
      "task_id": "clv-child-2",
      "purpose": "parallel subtask",
      "spec_binding": "auspex-data-model-v2"
    },
    "policy": {
      "provider": null,
      "model": null,
      "thinking_level": null,
      "context_class": null,
      "tool_policy": null,
      "memory_mode": null,
      "max_runtime_seconds": 900,
      "max_cost_usd": 0.50
    }
  },
  "observed": {
    "placement": {
      "placement_id": "pod/auspex/omegon-child-abc123",
      "host": "cluster:dev-us-east-1",
      "pid": null,
      "namespace": "auspex",
      "pod_name": "omegon-child-abc123",
      "container_name": "omegon"
    },
    "control_plane": {
      "schema_version": 2,
      "omegon_version": "0.15.7",
      "base_url": "http://omegon-child-abc123.auspex.svc:7842",
      "startup_url": "http://omegon-child-abc123.auspex.svc:7842/api/startup",
      "health_url": "http://omegon-child-abc123.auspex.svc:7842/api/healthz",
      "ready_url": "http://omegon-child-abc123.auspex.svc:7842/api/readyz",
      "ws_url": "ws://omegon-child-abc123.auspex.svc:7842/ws?token=...",
      "auth_mode": "ephemeral-bearer",
      "token_ref": "secret://auspex/instances/omg_01HV.../token",
      "last_ready_at": "2026-04-03T12:00:11Z"
    },
    "health": {
      "ready": true,
      "degraded_reason": null,
      "last_heartbeat_at": "2026-04-03T12:03:42Z"
    },
    "exit": {
      "exited": false,
      "exit_code": null,
      "exit_reason": null,
      "exited_at": null
    }
  }
}
```

## Minimal required fields

### Identity
- `instance_id`
- `role`
- `profile`
- `status`
- `created_at`
- `updated_at`

### Ownership
- `owner_kind`
- `owner_id`
- `parent_instance_id` (optional)

### Desired state
- backend kind
- workspace binding
- task binding (optional)
- policy overrides (optional)

### Observed state
- placement information
- control-plane endpoints and versions
- health state
- exit state

## Registry semantics

- `desired` is authoritative for reconciliation
- `observed` is authoritative for UI/debugging
- `status` in `identity` is a synthesized worker lifecycle label
- `observed.control_plane.token_ref` should point to a secret backend, keychain, or Kubernetes Secret reference
- session-level dispatcher attachment should resolve through both logical worker identity and authenticated control-plane verification

## Status vocabulary

Use:
- `requested`
- `allocating`
- `starting`
- `ready`
- `busy`
- `degraded`
- `stopping`
- `exited`
- `lost`
