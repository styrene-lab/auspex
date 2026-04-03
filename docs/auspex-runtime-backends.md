---
id: auspex-runtime-backends
title: "Auspex runtime backends for local, OCI, and Kubernetes workers"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions:
  - "What common instantiation contract normalizes local-process, local-detached, OCI, and Kubernetes workers?"
dependencies: []
related:
  - auspex-multi-agent-runtime
---

# Auspex runtime backends for local, OCI, and Kubernetes workers

## Overview

Define the backend abstraction for launching and reconciling Omegon workers as local subprocesses, detached local services, OCI containers, or Kubernetes-managed workloads.

## Decision

### Runtime backends share one logical worker contract

**Status:** accepted

Auspex should instantiate logical workers through a backend-agnostic request shape, then let the selected backend realize that worker in its own way.

## First-pass backend kinds

- `local-process`
- `local-detached`
- `oci-container`
- `kubernetes`

## First-pass instantiation contract

```json
{
  "role": "primary-driver | supervised-child | detached-service",
  "profile": "cheap-subtask",
  "backend": "local-process | local-detached | oci-container | kubernetes",
  "workspace": {
    "cwd": "/repo/path",
    "workspace_id": "repo:hash"
  },
  "parent_instance_id": "optional-parent-id",
  "task": {
    "task_id": "clv-child-2",
    "purpose": "parallel subtask"
  },
  "overrides": {
    "model": "anthropic:claude-haiku",
    "thinking_level": "low",
    "max_runtime_seconds": 900,
    "namespace": "auspex",
    "resources": {
      "cpu": "500m",
      "memory": "1Gi"
    }
  }
}
```

## Backend semantics

### `local-process`
- child process of current Auspex session
- best for low-latency interactive/delegated work
- strongest ownership semantics

### `local-detached`
- local background service with durable registry entry
- survives window restart
- may require a background Auspex supervisor/agent-manager process

### `oci-container`
- worker runs in a container runtime on the local/remote host
- stronger resource isolation than plain subprocess
- useful stepping stone toward cluster deployment

### `kubernetes`
- worker realized as a pod/job/deployment-backed runtime
- observed state must be reconciled asynchronously
- registry must track desired and observed state separately

## Constraint

Kubernetes is a first-class backend.
The schema and runtime API must not assume localhost PID+port is the only execution model.
