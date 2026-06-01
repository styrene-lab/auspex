+++
title = "Native Local Management Next Five"
tags = ["auspex","native","local-management","roadmap"]
+++

+++
id = "532c2135-e208-4517-b928-896d7fdae713"
kind = "design_node"

[data]
title = "Native Local Management Next Five"
status = "decided"
issue_type = "plan"
priority = 1
parent = "e815b23d-0986-4e4f-b143-f89e44f80432"
dependencies = []
open_questions = []
+++

## Overview

# Native Local Management Next Five

---
title: Native Local Management Next Five
status: decided
tags: [auspex, native, local-management, roadmap]
---

# Native Local Management Next Five

Parent: [[local-omegon-instance-management-mvp]]

The native local attach MVP now works end-to-end. These are the next five concrete slices, in order.

## 1. Normalize runtime role/profile projection

Problem: the same attached runtime appears as `primary_driver` in the top rail while the fleet table shows `DetachedService`.

Acceptance:

- Descriptor roles normalize snake_case and kebab-case consistently.
- `primary_driver` / `primary-driver` projects to `PrimaryDriver` everywhere.
- Fleet table, left rail, and top rail agree on role/profile labels.
- Tests cover role normalization for `primary_driver`, `primary-driver`, `supervised_child`, `detached_service`.

## 2. Ingest operational profile metadata from local attach

Problem: gateway degradation reports no operational profile metadata after successful attach.

Acceptance:

- Local attach probe captures operational/initialize/extension metadata when available.
- `observed.operational_profile` is populated via existing operational profile parser.
- Fleet projection no longer reports missing profile when metadata is present.
- Tests cover metadata-present and metadata-absent degradation behavior.

## 3. Map runtime HostAction support

Problem: gateway degradation reports no known HostAction support even though runtime capabilities include control capabilities such as `shutdown` and future HostAction surfaces.

Acceptance:

- Runtime capability evidence distinguishes first-party runtime controls from HostActions.
- `package.install@1` / explicit HostAction metadata maps to HostAction capability evidence.
- `shutdown` maps to runtime control capability, not automatically full HostAction support.
- Degradation copy distinguishes “no HostAction support” from “no mutating lifecycle support”.
- Tests cover `shutdown`, `package.install@1`, and no-host-action cases.

## 4. Add lifecycle controls for AuspexOwned only

Problem: attach/probe is read-only; lifecycle controls are not yet exposed and must not target user-owned runtimes.

Acceptance:

- Stop/restart buttons render only when selected runtime ownership is `AuspexOwned`.
- UserOwned/Unknown/External runtimes show read-only status with reason.
- Lifecycle actions require authorization approval + audit obligation.
- No lifecycle action is available from browser/wasm mode.
- Tests cover render/action gating and denied external stop.

## 5. Replace dev principal with real Styrene identity/RBAC principal

Problem: `attach_probe_principal()` is a temporary local dev principal.

Acceptance:

- Principal is resolved from `styrene-identity`/`styrene-rbac` where available.
- Anonymous fallback can discover but cannot attach/control/lifecycle.
- Capability grants map into `styrene-policy::PrincipalRef`.
- Missing signer blocks actions requiring signature.
- COP shows principal identity/role/capability summary for policy decisions.

## Open Questions
