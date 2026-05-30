+++
title = "Auspex Fleet Runtime Projection"
tags = ["auspex","fleet","projection"]
+++

+++
id = "4c02b62d-68dc-4c8a-8ce0-dd5ab318d47b"
kind = "design_node"

[data]
title = "Auspex Fleet Runtime Projection"
status = "exploring"
issue_type = "architecture"
priority = 2
dependencies = []
open_questions = []
+++

## Overview

# Auspex Fleet Runtime Projection

---
title: Auspex Fleet Runtime Projection
tags: [auspex, fleet, projection, flynt]
---

# Auspex Fleet Runtime Projection

## Overview

Auspex should expose its own live operational model as structured state/events. Flynt may project this state into workspace artifacts, but Auspex remains the authority for runtime orchestration semantics.

## Projection Targets

- Auspex native operator UI/COP.
- Flynt documents/graphs/design surfaces.
- Mobile/remote lightweight control surfaces.
- Kubernetes/operator status endpoints.

## Open Questions

- [assumption] Projection should be downstream-only by default: Flynt renders Auspex state, while mutations return through Auspex APIs/policies.
- What event schema should represent compatibility, capability drift, HostAction approvals, and dispatch state?
- Should projected Flynt artifacts be generated snapshots or live linked resources?

## First Implementation Slice

1. Define a fleet state summary struct independent of any one UI.
2. Include instance compatibility, profile, capability, HostAction queue, and audit summary fields.
3. Add serialization tests so Flynt/mobile projections consume stable JSON.

## Open Questions
