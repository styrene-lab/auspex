+++
title = "Native Attach and Probe Local Omegon Candidate"
tags = ["auspex","omegon","attach","probe"]
+++

+++
id = "911e30f1-dcbe-4ea5-bd57-b1594abd8224"
kind = "design_node"

[data]
title = "Native Attach and Probe Local Omegon Candidate"
status = "exploring"
issue_type = "implementation-slice"
priority = 2
parent = "577cd4ab-2324-4e88-bf91-345083d53131"
dependencies = []
open_questions = []
+++

## Overview

# Native Attach and Probe Local Omegon Candidate

---
title: Native Attach and Probe Local Omegon Candidate
status: exploring
tags: [auspex, omegon, attach, probe]
---

# Native Attach and Probe Local Omegon Candidate

Parent: [[native-local-management-mvp-implementation-plan]]

## Scope

Convert a discovered local candidate into a live `InstanceRecord` only after successful startup/state probing and compatibility validation.

## Probe Sequence

1. Resolve startup URL.
2. Fetch `/api/startup`.
3. Validate web startup schema and Omegon version.
4. Resolve state URL.
5. Fetch `/api/state`.
6. Parse state snapshot and instance descriptor.
7. Build/update `InstanceRecord`.
8. Populate compatibility/capabilities/profile fields.
9. Register attached instance route.

## Open Questions

- [assumption] Local candidates without startup URL can infer one from default known ports.
- [assumption] `/api/startup` remains the best compatibility gate for local attach.
- How should token-bearing local endpoints be discovered without leaking token values into COP?

## Tasks

- [ ] Define `LocalOmegonProbeResult`.
- [ ] Add `probe_local_omegon_candidate()`.
- [ ] Add controller method `attach_local_omegon_candidate()`.
- [ ] Add COP selected-candidate state or action payload.
- [ ] Add failure reason rendering for incompatible/unreachable candidates.

## Open Questions
