+++
title = "Auspex Armory/Nex Package Reconciliation"
tags = ["auspex","armory","nex","packages"]
+++

+++
id = "f73d0dff-017d-4476-a48f-76448a450ff8"
kind = "design_node"

[data]
title = "Auspex Armory/Nex Package Reconciliation"
status = "exploring"
issue_type = "architecture"
priority = 2
dependencies = []
open_questions = []
+++

## Overview

# Auspex Armory/Nex Package Reconciliation

---
title: Auspex Armory/Nex Package Reconciliation
tags: [auspex, armory, nex, packages, capabilities]
---

# Auspex Armory/Nex Package Reconciliation

## Overview

Auspex should promote the older Armory overlay preflight work into a desired/actual package and capability reconciliation lane. Omegon 0.25 splits read-only capability resolution from mutating package installation, which matches Auspex's orchestration role.

## Desired Flow

```text
desired capability/package state
→ read-only capability discovery via nex_capability / instance metadata
→ Armory/Nex artifact evidence and preflight
→ approved package.install@1 HostAction
→ capability verification
→ record actual instance state and audit trail
```

## Open Questions

- [assumption] Armory remains the catalog/inventory source for installable extension/package artifacts.
- What package identity fields must be recorded on Auspex instance records?
- How should Auspex reconcile package drift between desired state and instance-reported capabilities?

## First Implementation Slice

1. Define package/capability desired-state structs.
2. Map desired capability to read-only discovery evidence.
3. Add package-install HostAction planning records, without executing them yet.

## Open Questions
