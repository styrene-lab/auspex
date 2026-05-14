---
id: auspex-runtime-backends
title: "Auspex runtime backends for local, OCI, and Kubernetes workers"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions: []
dependencies: []
related:
  - auspex-multi-agent-runtime
---

# Auspex runtime backends for local, OCI, and Kubernetes workers

## Overview

Define the backend abstraction for launching and reconciling Omegon workers as local subprocesses, detached local services, OCI containers, or Kubernetes-managed workloads.

## Decisions

### Runtime backends share one instantiation request schema

**Status:** accepted

Auspex should instantiate logical workers through a backend-agnostic request shape, then let the selected backend realize the worker according to backend-specific semantics.

### Kubernetes is a first-class backend

**Status:** accepted

The runtime API and registry model must not assume localhost PID+port is the only execution model.

### Cluster deployments need an Auspex primary driver

**Status:** implemented in `auspex-operator`

When Auspex runs as a Kubernetes backend, the operator bootstraps a dedicated long-running `OmegonAgent` for the operator-facing session dispatcher. This is not a generic fleet member and not another local embedded runtime. It is the cluster-resident `primary-driver` that Chat, COP decisions, delegation, and future handoff surfaces attach to.

Defaults:

- `metadata.name`: `auspex-primary`
- `metadata.namespace`: `AUSPEX_PRIMARY_AGENT_NAMESPACE`, else `AUSPEX_WATCH_NAMESPACE`, else `omegon-agents`
- `spec.agent`: `styrene.auspex-primary`
- `spec.role`: `primary-driver`
- `spec.mode`: `daemon`
- `spec.posture`: `architect`
- `spec.model`: `AUSPEX_PRIMARY_AGENT_MODEL`, else `anthropic:claude-sonnet-4-6`
- `spec.image`: `AUSPEX_PRIMARY_AGENT_IMAGE`, else `ghcr.io/styrene-lab/omegon-agents:latest`

Set `AUSPEX_BOOTSTRAP_PRIMARY_AGENT=false` to disable bootstrap when GitOps owns the primary agent manifest. Set `AUSPEX_PRIMARY_AGENT_SECRET` to attach the Kubernetes Secret that carries provider credentials and runtime tokens.

The operator advertises daemon `OmegonAgent` control planes through `/api/fleet`, including the in-cluster service URL and ACP websocket URL. The UI can therefore distinguish the single operator-facing primary from supervised children and detached services.

## Deploy profiles

Deploy profiles define the backend, OCI image, resource requirements, and placement constraints. Schema lives in `pkl/DeployProfile.pkl`, config loaded from `~/.config/auspex/deploy-profiles.pkl` (toml fallback).

```pkl
amends "DeployProfile.pkl"

version = 1

profiles {
  ["local-default"] {
    backend = "local-process"
  }
  ["homelab-container"] {
    backend = "oci-container"
    image = "ghcr.io/styrene-lab/omegon:v0.15.25"
    namespace = "auspex"
    resources { cpu = "1"; memory = "2Gi" }
    restart_on_exit = true
  }
  ["k8s-worker"] {
    backend = "kubernetes"
    image = "ghcr.io/styrene-lab/omegon:v0.15.25"
    namespace = "agents"
    max_instances = 8
    resources { cpu = "500m"; memory = "1Gi" }
    requires { "kubectl"; "helm" }
  }
}
```

## Canonical instantiate request schema

This is the internal shape Auspex uses regardless of backend. It combines a resolved worker profile with a deploy profile.

```json
{
  "schema_version": 1,
  "role": "supervised-child",
  "profile": "cheap-subtask",
  "backend": "kubernetes",
  "workspace": {
    "cwd": "/repo/path",
    "workspace_id": "repo:8f2f4c1",
    "branch": "main"
  },
  "parent_instance_id": "omg_primary_01HV...",
  "task": {
    "task_id": "clv-child-2",
    "purpose": "parallel subtask",
    "spec_binding": "auspex-data-model-v2"
  },
  "overrides": {
    "model": "anthropic:claude-haiku",
    "thinking_level": "low",
    "max_runtime_seconds": 900,
    "image": "ghcr.io/styrene-lab/omegon:v0.15.25",
    "namespace": "auspex",
    "resources": {
      "cpu": "500m",
      "memory": "1Gi"
    }
  }
}
```

## Backend adapter expectations

### `local-process`
- returns child pid and local port
- strong session ownership
- fastest startup path

### `local-detached`
- returns durable local service identity
- may outlive the UI process
- should still register token and placement metadata

### `oci-container`
- launches through container runtime
- returns container id / mapped port / image info
- useful for local or remote host isolation

### `kubernetes`
- creates pod/job/deployment-backed runtime
- returns placement id, namespace, reconciliation handle, and daemon control-plane endpoints
- readiness becomes asynchronous and must update the registry later

## Reconciliation contract

Backends should support:
- `instantiate(request)`
- `observe(instance_id)`
- `stop(instance_id)`
- `reap(instance_id)`

This is enough for a first-pass supervisor implementation.
