+++
title = "Native Local Discovery and Ownership Enrichment"
tags = ["auspex","omegon","discovery","ownership"]
+++

+++
id = "9cc2c182-9521-4cc8-b7e2-f8bdf8d78556"
kind = "design_node"

[data]
title = "Native Local Discovery and Ownership Enrichment"
status = "decided"
issue_type = "implementation-slice"
priority = 1
parent = "577cd4ab-2324-4e88-bf91-345083d53131"
dependencies = []
open_questions = []
+++

## Overview

# Native Local Discovery and Ownership Enrichment

---
title: Native Local Discovery and Ownership Enrichment
status: decided
tags: [auspex, omegon, discovery, ownership]
---

# Native Local Discovery and Ownership Enrichment

Parent: [[native-local-management-mvp-implementation-plan]]

## Scope

Turn local discovery from a process-table parser into a real native discovery pass.

## Inputs

1. Auspex-owned PID file
2. Process table scan
3. Known local control ports
4. Configured local instances
5. Future: IPC sockets

## Output

A de-duplicated list of `LocalOmegonCandidate` records with explicit ownership.

## Decisions

- Process scanning remains non-mutating.
- PID-file evidence upgrades matching process candidates to `AuspexOwned`.
- Known-port candidates without matching process evidence are `Unknown` until probed.
- Candidate identity is `(pid)`, then `(state_url/startup_url)`, then `(ipc_socket)`.
- This slice does not attach or command runtimes.

## Tasks

- [x] Add `discover_owned_pid_candidate()`.
- [x] Add `discover_known_control_port_candidates()`.
- [x] Add `merge_local_omegon_candidates()`.
- [x] Add `discover_local_omegon_candidates()` orchestration function.
- [x] Update controller to use aggregate discovery, not only process table.
- [x] Add unit tests for ownership upgrade, URL de-duplication, and unknown known-port candidates.

## Open Questions
