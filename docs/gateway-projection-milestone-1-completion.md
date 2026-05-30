+++
title = "Gateway Projection Milestone 1 Completion Plan"
tags = ["auspex","gateway","fleet","projection"]
+++

+++
id = "b011cf49-6805-4f79-a988-eaf4d06fb118"
kind = "design_node"

[data]
title = "Gateway Projection Milestone 1 Completion Plan"
status = "implementing"
issue_type = "implementation"
priority = 1
dependencies = []
open_questions = []
+++

## Overview

# Gateway Projection Milestone 1 Completion Plan

---
title: Gateway Projection Milestone 1 Completion Plan
status: implementing
tags: [auspex, gateway, fleet, projection, milestone]
---

# Gateway Projection Milestone 1 Completion Plan

## Goal

Complete the first Auspex-as-gateway milestone: read-only projection of the deployed fleet. This milestone must not introduce dispatch, HostAction execution, package installation, or transport-specific side effects.

## Scope

Finish the internal gateway projection surface for:

```text
auspex/fleet/status
auspex/instances/list
auspex/capabilities/query
```

The API remains an internal Rust/controller API for now. Transport exposure comes after DTO stability.

## Remaining implementation deltas

1. Namespace-aware capability classification.
2. Gateway method registry / canonical method names.
3. Richer degradation signals:
   - empty fleet;
   - no compatible instances;
   - unsupported instances;
   - not-ready instances;
   - missing operational profile;
   - no known HostAction support.
4. Stable serialization tests for projection DTOs.
5. Controller tests proving gateway methods derive from registry projection.

## Adversarial assessment criteria

Reject the implementation if it:

- invents separate gateway state instead of deriving from `FleetRuntimeProjection`;
- performs mutation or dispatch;
- treats ACP permission/capability as authority;
- hides unsupported/degraded instances;
- collapses `auspex/*`, `omegon/*`, `styrene/*`, HostAction, Nex, Armory, and generic tool capabilities into indistinguishable strings;
- produces non-deterministic JSON output for the same fleet state;
- makes empty fleet look healthy;
- makes a fleet with no compatible instances look degraded rather than unsupported.

## Decisions

- Milestone 1 remains read-only and internal-API-first.
- Canonical gateway method names are explicit constants and enum variants.
- Capability namespace is derived from capability keys at projection time; existing capability evidence remains backward-compatible.
- Degradation is derived, not stored, until transport/client negotiation exists.

## Open Questions
