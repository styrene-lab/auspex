---
id: auspex-cnpg-persistence
title: "Auspex Kubernetes persistence with CloudNativePG"
status: proposed
parent: auspex-runtime-backends
tags: [auspex, kubernetes, postgres, cloudnativepg, persistence]
open_questions:
  - "Should the first Auspex chart install the CloudNativePG operator as a dependency, or require it as a cluster prerequisite?"
  - "Which Auspex state becomes Postgres-backed first: instance registry, workflow runs, telemetry rollups, or durable queue/outbox state?"
dependencies:
  - auspex-runtime-backends
  - auspex-instance-registry-schema
  - auspex-telemetry-aggregation
related:
  - auspex-multi-agent-runtime
  - project-manifest-and-registry
---

# Auspex Kubernetes persistence with CloudNativePG

## Context

This document adapts the PostgreSQL HA validation work to Auspex specifically.
It is not a generalized appliance-control-plane design. Auspex is the
supervisor/gateway for Omegon workers and long-running agent sessions, so the
persistence question should be scoped to that role:

- track desired and observed worker state;
- preserve dispatcher and detached-service metadata across restarts;
- support workflow graph/run state;
- aggregate telemetry without forcing Omegon to become an observability backend;
- provide durable queue/outbox semantics for orchestration work.

The earlier bare-metal Spilo/Patroni/etcd/HAProxy/Keepalived stack is useful as
an airgapped infrastructure pattern, but it should not be the default Auspex
Kubernetes shape.

## Current Auspex Fit

Auspex already treats Kubernetes as a first-class runtime backend. The operator
owns `OmegonAgent` resources and reconciles them into Kubernetes primitives:

- daemon agents become `Deployment` + `Service`;
- bounded agents become `Job`;
- scheduled agents become `CronJob`;
- the primary driver is a dedicated `OmegonAgent` with role
  `primary-driver`;
- ExternalAgent exists for monitoring and proxying agents outside the cluster.

The accepted runtime model is still: Auspex supervises logical Omegon workers;
Omegon remains the worker/runtime. PostgreSQL should therefore support Auspex
state, not replace the worker model.

## Recommendation

For Kubernetes deployments, use CloudNativePG directly instead of carrying the
bare-metal Spilo appliance stack into the Auspex chart.

CloudNativePG is the right default for in-cluster Auspex because:

- it is Kubernetes-native and uses Kubernetes resources/API conventions instead
  of requiring separate etcd, HAProxy, and Keepalived layers;
- it provides a `Cluster` custom resource that fits the same declarative model
  as Auspex's `OmegonAgent` CRDs;
- it supports object-store backup/recovery through Barman Cloud, including S3
  compatible targets such as MinIO;
- it keeps the application contract as ordinary PostgreSQL, preserving a future
  migration path from bare metal or back to bare metal.

The Spilo appliance model should remain a separate non-Kubernetes deployment
option for constrained bare metal. It is too much machinery for the Auspex
chart when Kubernetes is already present.

## Is Postgres Needed Immediately?

Not for the local desktop product path.

Auspex can continue using local files/SQLite-style storage for:

- a single desktop operator;
- local-only instance registry records;
- short-lived development sessions;
- mock or fixture-driven UI work.

PostgreSQL becomes worthwhile for the Kubernetes deployment path when Auspex is
expected to supervise long-running agents across restarts and multiple
controllers. The first real need is not "database because app"; it is durable
orchestration state:

- primary-driver identity and handoff state;
- detached-service ownership and reattach metadata;
- desired/observed worker reconciliation history;
- workflow graph run state;
- durable queue/outbox for orchestration commands;
- bounded telemetry rollups for fleet and session views.

Therefore the chart should make Postgres optional at first:

```yaml
persistence:
  enabled: false
  backend: local
```

Then the Kubernetes profile can enable it:

```yaml
persistence:
  enabled: true
  backend: postgres
  postgres:
    mode: cloudnativepg
```

## Proposed Chart Shape

Auspex does not currently appear to have a Helm chart in this repository. When
one is added, the persistence section should be explicit and opt-in.

### Values

```yaml
persistence:
  enabled: true
  backend: postgres

  postgres:
    mode: cloudnativepg
    existingSecret: auspex-postgres-app
    database: auspex
    owner: auspex

    cloudnativepg:
      createCluster: true
      clusterName: auspex-postgres
      instances: 3
      storage:
        size: 20Gi
        storageClass: ""
      resources:
        requests:
          cpu: 250m
          memory: 512Mi
        limits:
          memory: 2Gi
      backups:
        enabled: false
        destinationPath: s3://auspex-postgres/
        endpointURL: http://minio.minio.svc:9000
        secretName: auspex-postgres-backup
        retentionPolicy: 30d
```

### CloudNativePG Cluster Sketch

```yaml
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: auspex-postgres
spec:
  instances: 3

  storage:
    size: 20Gi

  bootstrap:
    initdb:
      database: auspex
      owner: auspex
      secret:
        name: auspex-postgres-app

  backup:
    barmanObjectStore:
      destinationPath: s3://auspex-postgres/
      endpointURL: http://minio.minio.svc:9000
      s3Credentials:
        accessKeyId:
          name: auspex-postgres-backup
          key: ACCESS_KEY_ID
        secretAccessKey:
          name: auspex-postgres-backup
          key: ACCESS_SECRET_KEY
    retentionPolicy: 30d
```

The Auspex deployment then receives a normal application DSN from a Secret:

```yaml
env:
  - name: AUSPEX_DATABASE_URL
    valueFrom:
      secretKeyRef:
        name: auspex-postgres-app
        key: uri
```

The exact Secret keys should follow whatever CNPG emits or whatever the chart
creates explicitly. The operator should not synthesize database passwords in
controller memory.

## Operator Integration

Auspex should not make every `OmegonAgent` talk directly to the database by
default. The database belongs first to the Auspex control/orchestration plane.

Recommended first integration:

1. Auspex operator reads `AUSPEX_DATABASE_URL` if present.
2. If absent, it stays in local/ephemeral mode.
3. If present, it runs migrations and stores Auspex-owned orchestration state.
4. Omegon worker pods remain database-agnostic unless a specific agent profile
   declares a Postgres dependency.

This keeps the worker boundary clean. Auspex persists orchestration state;
workers expose control planes and emit telemetry.

## Candidate Auspex Tables

The first schema should mirror existing Auspex concepts rather than introduce a
generic control-plane database.

```text
auspex_instances
auspex_instance_observations
auspex_control_planes
auspex_workflow_runs
auspex_workflow_nodes
auspex_queue_jobs
auspex_queue_attempts
auspex_outbox_events
auspex_telemetry_rollups
auspex_schema_migrations
```

### Instance Registry Mapping

`auspex_instances` should persist the same split already described in
`auspex-instance-registry-schema`:

- identity;
- ownership;
- desired backend/workspace/task/policy;
- observed placement/control-plane/health/exit state.

The database should be a durable backing store for that model, not a competing
model.

### Durable Queue / Outbox

For orchestration commands and workflow graph execution, use transactional
PostgreSQL queues:

```sql
with claimed as (
  select id
  from auspex_queue_jobs
  where status = 'pending'
  order by id
  for update skip locked
  limit 1
)
update auspex_queue_jobs q
set status = 'running',
    locked_at = now(),
    locked_by = $1
from claimed
where q.id = claimed.id
returning q.*;
```

External side effects should go through an outbox table so reconcile loops can
retry safely after controller restarts.

## Degradation Model

Auspex should degrade by deployment mode:

### Desktop/local

No Postgres required. Use local registry files and existing local persistence.

### Single-node development cluster

Optional CNPG single-instance or external Postgres is acceptable for testing,
but documentation should call it non-HA.

### Production Kubernetes

Use CNPG with at least three instances when the cluster has enough nodes and
storage topology to make that meaningful.

### Bare-metal without Kubernetes

Use the separate Spilo/Patroni/etcd/HAProxy/Keepalived appliance design. Do not
force that design into the Auspex chart.

## Backup and Recovery

For the Auspex chart, prefer CNPG's Barman Cloud integration over hand-rolled
WAL-G sidecars. A local MinIO endpoint can satisfy the same airgap requirement
without changing the database layer.

Minimum production expectations:

- object-store backups enabled;
- restore drill documented;
- retention policy explicit;
- backup credentials supplied by Secret or external secret manager;
- no backup keys embedded in Helm values committed to git.

## Security Boundary

The Auspex operator already has a strong secret posture in the CRD surface:
tokens and auth material are referenced through Secrets or Vault mappings rather
than placed inline in specs.

The database design should preserve that:

- no provider API keys in Postgres;
- no WebSocket bearer tokens inline unless encrypted or represented as a secret
  reference;
- no raw prompts in telemetry tables by default;
- bounded-cardinality telemetry rollups only;
- instance records may store endpoint metadata, but auth material should remain
  a Secret/Vault reference.

## Migration Path

1. Add a persistence config surface to the future Auspex chart.
2. Add optional CNPG `Cluster` rendering or document CNPG as a prerequisite.
3. Add `AUSPEX_DATABASE_URL` support to `auspex-operator`.
4. Add migrations for instance registry persistence.
5. Move workflow run state into Postgres.
6. Add queue/outbox tables for orchestration commands.
7. Add telemetry rollup tables last, after cardinality/redaction rules are
   settled.

This sequence avoids blocking current runtime work on database plumbing.

## Decision

Use CloudNativePG directly for Kubernetes Auspex deployments.

Do not embed the full Spilo/Patroni/etcd/HAProxy/Keepalived stack into Auspex's
Kubernetes chart. Keep that stack as the non-Kubernetes bare-metal option.

Make Postgres optional until the Kubernetes deployment profile needs durable
multi-agent orchestration state. Once enabled, scope it to Auspex-owned state
first, not arbitrary agent application data.

## References

- CloudNativePG documentation: https://cloudnative-pg.io/documentation/
- CloudNativePG installation and operator docs: https://cloudnative-pg.io/documentation/current/installation_upgrade/
- CloudNativePG backup and recovery docs: https://cloudnative-pg.io/documentation/current/backup/
- CloudNativePG bootstrap/recovery docs: https://cloudnative-pg.io/documentation/current/bootstrap/

