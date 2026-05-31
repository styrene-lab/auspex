+++
title = "Local Omegon Instance Management MVP"
tags = ["auspex","omegon","mvp","orchestration"]
+++

+++
id = "e815b23d-0986-4e4f-b143-f89e44f80432"
kind = "design_node"

[data]
title = "Local Omegon Instance Management MVP"
status = "exploring"
issue_type = "mvp"
priority = 1
dependencies = []
open_questions = []
+++

## Overview

# Local Omegon Instance Management MVP

---
title: Local Omegon Instance Management MVP
status: exploring
tags: [auspex, omegon, mvp, orchestration]
---

# Local Omegon Instance Management MVP

Auspex's next MVP is live management of Omegon runtimes on the local machine. Demo fleet data is a development aid, not the product path.

## Goal

Auspex can discover, attach to, observe, command, and safely lifecycle-manage local Omegon instances.

## Slice 1 — Local Runtime Discovery Model

Create a machine-readable discovery layer that enumerates local Omegon candidates before any attach/control operation.

Discovery sources:

- Auspex-owned PID file
- process table scan
- known local control ports
- configured local entries
- IPC/socket paths when available

## Safety Rules

- Auspex-owned processes may be lifecycle-managed.
- User-owned processes may be observed/attached when compatible, but not killed/restarted by default.
- Unknown candidates are probe-only.
- Incompatible Omegon versions are displayed as unsupported and not commanded.

## Acceptance Criteria

- A typed `LocalOmegonCandidate` model exists.
- Candidate ownership is explicit: `AuspexOwned`, `UserOwned`, or `Unknown`.
- Process-table parsing is unit-tested without relying on the host process list.
- Owned PID file discovery is represented separately from process scanning.
- Discovery code is non-mutating.

## Open Questions
