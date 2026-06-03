+++
title = "Auspex Embedded Central Runtime"
tags = ["auspex","omegon","embedded-runtime","future"]
+++

+++
id = "057cb2da-c1cc-405a-a41d-2b2a85e0c099"
kind = "design_node"

[data]
title = "Auspex Embedded Central Runtime"
status = "exploring"
issue_type = "architecture"
priority = 2
parent = "e815b23d-0986-4e4f-b143-f89e44f80432"
dependencies = []
open_questions = []
+++

## Overview

# Auspex Embedded Central Runtime

---
title: Auspex Embedded Central Runtime
status: exploring
tags: [auspex, omegon, embedded-runtime, future]
---

# Auspex Embedded Central Runtime

Parent: [[local-omegon-instance-management-mvp]]

## Future target

Native Auspex may eventually spawn and own a central embedded Omegon runtime for its own orchestration loop.

This is distinct from the current MVP, which only observes local runtimes.

## Intended future semantics

The embedded runtime would be:

```text
ownership   = AuspexOwned
observation = ProbedFresh
authority   = CommandAuthorized / LifecycleAuthorized
```

It may coordinate:

- observed local runtimes
- Nex sandboxes
- child/worker Omegon runtimes
- evidence/project-rules validation
- lifecycle operations for Auspex-owned runtimes

## Non-goal for current MVP

The current local runtime observation path must not pretend this central runtime exists.

Current MVP:

```text
observe → probe → project → persist stale observation → reprobe
```

Future embedded path:

```text
spawn central runtime → own handle → attach/probe → command/lifecycle authority
```

## Dependencies

- [[runtime-observation-authority-invariant]]
- [[local-attach-persistence-and-rehydration]]
- [[native-auspex-nex-sandbox-dogfood-lane]]
- authorization substrate from [[auspex-authorization-recommendation]]
- lifecycle controls gated to AuspexOwned runtimes

## Open questions

- Should the embedded runtime be launched directly by Auspex or by Nex as a sandboxed process?
- Which credentials does the embedded runtime receive by default?
- Does central runtime authority survive app restart, or must it always be reacquired?
- What is the minimum evidence emitted by the embedded runtime before it can coordinate children?

## Open Questions
