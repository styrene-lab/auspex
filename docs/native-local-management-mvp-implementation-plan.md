+++
title = "Native Local Management MVP Implementation Plan"
tags = ["auspex","omegon","native","mvp","implementation"]
+++

+++
id = "577cd4ab-2324-4e88-bf91-345083d53131"
kind = "design_node"

[data]
title = "Native Local Management MVP Implementation Plan"
status = "implementing"
issue_type = "implementation-plan"
priority = 1
parent = "e815b23d-0986-4e4f-b143-f89e44f80432"
dependencies = []
open_questions = []
+++

## Overview

# Native Local Management MVP Implementation Plan

---
title: Native Local Management MVP Implementation Plan
status: implementing
tags: [auspex, omegon, native, mvp, implementation]
---

# Native Local Management MVP Implementation Plan

Parent: [[local-omegon-instance-management-mvp]]

## Decision

The MVP target is the native/thick Dioxus client. Browser/server mode is deferred. The native client is the correct first authority because it can inspect local process state, PID files, localhost endpoints, IPC sockets, and owned child processes directly.

## Product Definition

Auspex native client manages local Omegon runtimes through four operator verbs:

1. **Discover** — enumerate local Omegon candidates without mutation.
2. **Attach** — probe compatible candidates and register live instances.
3. **Launch** — start an Auspex-owned Omegon runtime and attach to it.
4. **Stop/Restart Owned** — lifecycle-manage only Auspex-owned runtimes.

## Safety Model

| Candidate ownership | Observe | Attach | Command | Stop/restart |
|---|---:|---:|---:|---:|
| Auspex-owned | yes | yes | yes, if compatible | yes |
| User-owned | yes | yes, if compatible | yes, if transport/token allows | no by default |
| Unknown | yes | probe-only | no | no |
| Unsupported version | yes | no | no | no |

## Implementation Slices

### Slice A — Native discovery COP

Status: partially implemented.

- [x] Add `LocalOmegonCandidate` model.
- [x] Parse `ps` output for `omegon serve` commands.
- [x] Render local discovery candidates into COP.
- [x] Replace primary demo button with `Discover Local`.
- [ ] Validate in native Dioxus client, not wasm.
- [ ] Add browser fallback copy: native discovery unavailable in browser mode.

### Slice B — Candidate enrichment

- [ ] Add Auspex-owned PID-file candidate source.
- [ ] Add known control-port candidate source (`7842` first, configured ports later).
- [ ] Add candidate de-duplication by PID/startup URL/state URL.
- [ ] Mark process-table candidates that match the owned PID as `AuspexOwned`.
- [ ] Add tests for de-duplication and ownership upgrade.

### Slice C — Attach/probe candidate

- [ ] Add `probe_local_omegon_candidate(candidate)`.
- [ ] Probe `/api/startup` and `/api/state`.
- [ ] Validate Omegon `0.25.x+` compatibility.
- [ ] Convert successful probe into `InstanceRecord`.
- [ ] Populate compatibility, capability snapshot, and operational profile metadata when available.
- [ ] COP action: `Attach Selected`.

### Slice D — Native launch

- [ ] Expose explicit `Launch Omegon` operator action.
- [ ] Use existing `spawn_and_attach_omegon`/bootstrap code path where possible.
- [ ] Record owned PID and ownership metadata.
- [ ] Attach launched runtime into registry.
- [ ] Show owned runtime as manageable in COP/sidecar.

### Slice E — Stop/restart owned

- [ ] Add `Stop Owned` action gated by `AuspexOwned`.
- [ ] Add `Restart Owned` action gated by `AuspexOwned`.
- [ ] Refuse lifecycle mutation for user-owned/unknown candidates.
- [ ] Record lifecycle actions into audit timeline.

### Slice F — Browser/server future path

Deferred.

- [ ] Add Dioxus server-function or local API bridge.
- [ ] Browser `Discover Local` calls bridge instead of native Rust.
- [ ] Keep same DTOs and COP rendering.

## Immediate Next Step

Run and manage the native client as the primary test surface, then complete Slice B.

```bash
cargo run
```

Expected operator-visible result after Slice B:

```text
Discover Local → COP shows real local candidates from process table, owned PID, and known port probes.
```

## Open Questions
