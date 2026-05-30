+++
title = "Auspex Parity Upgrade Program"
tags = ["auspex","omegon-025","acp","parity"]
+++

+++
id = "9ba14aff-d5db-42d6-8867-b05ffbbaa9a1"
kind = "design_node"

[data]
title = "Auspex Parity Upgrade Program"
status = "exploring"
issue_type = "architecture"
priority = 1
dependencies = []
open_questions = []
+++

## Overview

# Auspex Parity Upgrade Program

---
title: Auspex Parity Upgrade Program
status: exploring
tags: [auspex, omegon-025, acp, orchestration, parity]
---

# Auspex Parity Upgrade Program

## Overview

Bring Auspex back into parity with the current Omegon/Flynt/Armory integration model. The upgrade centers on Omegon 0.25 ACP operational profiles, capability discovery, HostAction policy, and Armory/Nex package reconciliation.

## Child Workstreams

- [[auspex-025-compatibility-capability-registry|Omegon 0.25 Compatibility and Capability Registry]]
- [[auspex-acp-operational-profile-adoption|ACP Operational Profile Adoption]]
- [[auspex-host-action-policy-gates|HostAction Policy Gates]]
- [[auspex-armory-nex-package-reconciliation|Armory/Nex Package Reconciliation]]
- [[auspex-fleet-runtime-projection|Fleet Runtime Projection]]

## Decisions

- Auspex owns live orchestration semantics for Omegon instances: fleet state, compatibility, supervision, dispatch, capabilities, HostAction policy, and audit.
- Flynt-agent is the adoption pattern for clean ACP profile metadata; Flynt is not the operational authority for Auspex.
- Auspex must stop inferring operational meaning from tool names alone. Operational profile, capabilities, and policy must be machine-readable at handshake time.

## Open Questions

- Auspex requires Omegon 0.25.x+ for this parity program; older incompatible releases such as 0.23 are unsupported rather than degraded.
- [assumption] Omegon 0.25 still speaks control-plane schema 2 for the surfaces Auspex currently consumes.
- What is the minimal runtime endpoint/handshake Auspex can implement first without blocking on deeper ACP proxy work?

## Open Questions
