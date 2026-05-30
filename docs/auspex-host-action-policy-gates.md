+++
title = "Auspex HostAction Policy Gates"
tags = ["auspex","host-actions","security"]
+++

+++
id = "a17a92e4-1b2f-48dc-9729-ceae25d75237"
kind = "design_node"

[data]
title = "Auspex HostAction Policy Gates"
status = "exploring"
issue_type = "security"
priority = 1
dependencies = []
open_questions = []
+++

## Overview

# Auspex HostAction Policy Gates

---
title: Auspex HostAction Policy Gates
tags: [auspex, host-actions, security, omegon-025]
---

# Auspex HostAction Policy Gates

## Overview

Auspex must classify Omegon HostActions before forwarding or approving them. Omegon 0.25.4 makes `package.install@1` a mutating HostAction backed by host-owned policy and managed terminal execution. Auspex must treat this as an operational security boundary.

## Policy Model

Initial classes:

- `read_only_discovery`: allowed as evidence, no mutation.
- `mutating_requires_approval`: can proceed only with operator or configured policy approval.
- `unsupported`: known but unavailable in this Auspex deployment.
- `deny`: unknown or disallowed by policy.

Initial mappings:

- `nex_capability` tool/action: read-only discovery evidence.
- `package.install@1`: mutating, requires approval and audit.
- unknown HostAction type: deny by default.

## Open Questions

- [assumption] Auspex approval queue should preserve the raw HostAction candidate and the normalized policy decision.
- Which existing audit timeline model should record approval/rejection/execution events?
- Should approval policy live in instance profile metadata, Auspex global config, or both?

## First Implementation Slice

1. Add a small typed HostAction policy classifier in `auspex-core`.
2. Add unit tests for `package.install@1`, read-only discovery, and unknown action denial.
3. Wire classifier output into later compatibility/capability registry work.

## Open Questions
