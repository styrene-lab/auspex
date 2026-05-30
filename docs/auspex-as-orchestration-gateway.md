+++
title = "Auspex as Orchestration Gateway"
tags = ["auspex","gateway","fleet","projection"]
+++

+++
id = "0016ff39-6d1d-44a7-9633-9fe50cadd52f"
kind = "design_node"

[data]
title = "Auspex as Orchestration Gateway"
status = "exploring"
issue_type = "architecture"
priority = 1
dependencies = []
open_questions = []
+++

## Overview

# Auspex as Orchestration Gateway

---
title: Auspex as Orchestration Gateway
status: exploring
tags: [auspex, gateway, fleet, projection, dispatch, acp, styrene]
---

# Auspex as Orchestration Gateway

## Overview

Auspex should serve as the fleet orchestration gateway. Omegon provides runtime-local gateway primitives; Auspex composes those primitives into a fleet-level gateway for clients, operators, workers, and Styrene mesh peers.

ACP remains the compatibility floor. First-party clients negotiate richer `auspex/*`, proxied `omegon/*`, and authority-bearing `styrene/*` surfaces through Auspex.

## Target posture

```text
Flynt / TUI / Zed / mobile / mesh peer / child agent
        ↓
Auspex Gateway
  - negotiation
  - degradation
  - policy
  - dispatch
  - projection
  - audit
        ↓
Primary embedded Omegon runtime
        ↓
Worker Omegon instances / Armory / Nex / Styrene mesh
```

## Milestones

### Milestone 1 — Projection of deployed fleet

Status: next target.

Expose a read-only gateway projection of deployed fleet state:

- known Omegon instances;
- compatibility status;
- degradation mode;
- advertised descriptor capabilities;
- operational profile metadata;
- HostAction support and policy class;
- package/capability evidence where known;
- route/placement/health summaries.

Candidate first methods:

```text
auspex/fleet/status
auspex/instances/list
auspex/capabilities/query
```

Success criteria:

- projection derives from `FleetRuntimeProjection`, not duplicated state;
- projection is read-only;
- unknown/degraded/unsupported surfaces are explicit;
- output is stable enough for COP/Flynt/mobile consumers;
- tests cover empty fleet, compatible fleet, degraded/unsupported instance, and capability evidence.

### Milestone 2 — Dispatch gateway

Status: next after projection.

Route operator/client intents through Auspex to the selected runtime:

- primary embedded Omegon;
- selected worker Omegon;
- future Styrene mesh peer;
- degraded ACP-only fallback where applicable.

Candidate methods:

```text
auspex/dispatch/submit
auspex/instances/select
auspex/routes/list
```

Success criteria:

- dispatch refuses unsupported/degraded targets unless a fallback is explicit;
- route selection uses compatibility/capability evidence;
- policy reasons are machine-readable;
- dispatch events are auditable;
- generic ACP clients degrade to primary-session behavior only.

## Roadmap after milestones

### Milestone 3 — HostAction approval gateway

- surface approval queue;
- model ACP permission selection as policy input, not authority;
- route package/terminal/browser HostActions through approval/audit;
- deny unknown HostActions by default.

### Milestone 4 — Armory/Nex package reconciliation

- desired package/capability state;
- read-only Nex capability evidence;
- approved `package.install@1` execution;
- post-install capability verification;
- drift reporting.

### Milestone 5 — Styrene identity/RBAC/mesh authority

- identity-bound client/session trust;
- RBAC role/capability grants;
- workspace lease authority;
- mesh route/delegation envelopes;
- first-party `styrene/*` method negotiation.

## Open Questions

- [assumption] Auspex should expose the first projection gateway over an internal Rust API before HTTP/WebSocket/ACP transport.
- [assumption] The first projection method set should be read-only and require no new HostAction execution.
- Which projection DTO shape is stable enough for COP/Flynt/mobile without leaking raw registry internals?
- Should degradation mode be introduced before projection transport, or initially derived in projection from compatibility/profile/capability state?
- How should generic ACP clients see the fleet gateway: single primary runtime only, or an explicit unsupported/degraded response for fleet methods?

## Decisions so far

- Auspex serves as the fleet orchestration gateway.
- Omegon provides runtime-local gateway primitives.
- ACP is the compatibility floor, not the complete Auspex gateway protocol.
- Milestone 1 is projection of deployed fleet.
- Milestone 2 is dispatch gateway.
- Mutation/HostAction/package reconciliation comes after read-only projection and dispatch are stable.

## Open Questions
