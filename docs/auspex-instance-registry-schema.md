---
id: auspex-instance-registry-schema
title: "Auspex instance registry schema"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions:
  - "What exact fields belong in desired state versus observed state for a worker instance?"
dependencies: []
related:
  - auspex-multi-agent-runtime
---

# Auspex instance registry schema

## Overview

Define the persistent schema for tracking logical Omegon workers across local, detached, OCI, and Kubernetes backends, including identity, placement, control-plane connectivity, lifecycle, and ownership.

## Decision

### Registry must describe logical workers, not only local processes

**Status:** accepted

The registry schema must be backend-agnostic. Local PID/port details are important, but insufficient once workers can also run as containers or Kubernetes workloads.

## First-pass schema shape

A registry record has five sections.

### 1. Identity

```json
{
  "instance_id": "omg_01HV...",
  "role": "primary-driver | supervised-child | detached-service",
  "profile": "primary-interactive | supervisor-heavy | cheap-subtask | background-service",
  "status": "requested | allocating | starting | ready | busy | degraded | stopping | exited | lost"
}
```

### 2. Ownership

```json
{
  "ownership": {
    "owner_kind": "auspex-session | auspex-daemon | k8s-controller | external",
    "owner_id": "...",
    "parent_instance_id": "optional-instance-id"
  }
}
```

### 3. Placement

```json
{
  "backend": {
    "kind": "local-process | local-detached | oci-container | kubernetes",
    "placement_id": "...",
    "host": "localhost | node-name | cluster-name",
    "pid": 12345,
    "namespace": "auspex",
    "pod_name": "auspex-worker-...",
    "container_name": "omegon"
  }
}
```

Backend-specific fields may be null/omitted where irrelevant.

### 4. Control plane

```json
{
  "control_plane": {
    "schema_version": 2,
    "omegon_version": "0.15.7",
    "base_url": "http://127.0.0.1:7842",
    "startup_url": "http://127.0.0.1:7842/api/startup",
    "health_url": "http://127.0.0.1:7842/api/healthz",
    "ready_url": "http://127.0.0.1:7842/api/readyz",
    "ws_url": "ws://127.0.0.1:7842/ws?token=...",
    "auth_mode": "ephemeral-bearer",
    "token_ref": "secret://auspex/instances/omg_01HV/token",
    "last_ready_at": "2026-04-03T12:00:00Z"
  }
}
```

### 5. Workload/policy binding

```json
{
  "workspace": {
    "cwd": "/repo/path",
    "workspace_id": "repo:hash",
    "branch": "main"
  },
  "task": {
    "task_id": "clv-child-2",
    "purpose": "parallel subtask",
    "spec_binding": "auspex-data-model-v2"
  }
}
```

## Desired vs observed state

The registry should distinguish:

- **desired state** — what Auspex asked for
- **observed state** — what actually exists now

That matters especially for OCI/Kubernetes workers where reconciliation is asynchronous.

### Desired state examples

- requested backend kind
- requested role/profile
- requested resources/limits
- requested image/version

### Observed state examples

- actual pid or pod name
- actual base URL and token ref
- actual readiness/liveness timestamps
- exit status / failure reason

## Constraints

- Do not store raw auth tokens inline if a secret reference can be used instead.
- Preserve enough metadata to reattach a detached worker after Auspex restart.
- Keep the schema compatible with both local and Kubernetes backends.
