---
title: Handoff — Native Runtime Observation MVP
status: current
created: 2026-06-02
tags: [handoff, auspex, omegon, native, runtime-observation]
---

# Handoff — Native Runtime Observation MVP

## Current product state

Native Auspex can observe a local Omegon runtime through a policy-gated, read-only probe path.

Current dogfood runtime:

```text
Omegon instance: web-compat
Omegon version: 0.25.4 observed locally; traits/source pin now 0.26.0
Startup URL: http://127.0.0.1:7842/api/startup
State URL:   http://127.0.0.1:7842/api/state
Raw role: primary_driver
Interpreted role: primary-driver
Raw profile: long-running-daemon
Raw runtime profile: primary_interactive
Compatibility: Compatible
Authority: ReadOnly observation, not command/lifecycle authority
```

The native UI now shows:

```text
Discover Local
Probe Runtime
Refresh Fleet
```

Terminology was corrected away from “attach” where the operation is only observation.

## Core invariant

See [[runtime-observation-authority-invariant]].

```text
observed runtime != command authority
persisted runtime evidence != fresh authority
AuspexOwned runtime != arbitrary local runtime
```

For the current local observed runtime:

```text
observation = ProbedFresh
ownership = OperatorOwned/External
runtime authority = ReadOnly
lifecycle authority = None
```

## Important design nodes

- [[local-omegon-instance-management-mvp]] — parent MVP
- [[runtime-observation-authority-invariant]] — decided invariant
- [[auspex-embedded-central-runtime]] — future embedded central runtime, not current MVP
- [[local-attach-persistence-and-rehydration]] — next implementation slice, name still has old attach wording
- [[native-auspex-nex-sandbox-dogfood-lane]] — future Nex/Omegon sandbox dogfood lane
- [[native-local-management-next-five]] — roadmap with sandbox dogfood added
- [[auspex-authorization-recommendation]] — styrene-policy/styrene-rbac authorization decision

## Recent commits of interest

```text
d7f8ae2 fix(ui): rename attach language to observation
abff5d2 docs(orchestration): separate runtime observation from authority
7bb61f5 fix(web): inject stylesheet before launch
7868430 feat(cop): expose raw runtime descriptor fields
782e7e7 fix(orchestration): preserve attached registry roles
6fe6f28 feat(orchestration): derive operational profile from local runtime
6e6ca04 feat(orchestration): track omegon evidence substrate capabilities
8b134bd docs(orchestration): add nex sandbox dogfood lane
0ae48bb feat(orchestration): persist local attach probe registry
```

## Validation status

Recent validation passed:

```text
cargo test -p auspex-core --lib -- --nocapture
cargo check
```

Most recent full core count seen:

```text
276 passed
```

`cargo check` also passed after committing the web stylesheet injection.

## Current dirty state expected

Only Flynt local metadata should remain untracked:

```text
.flynt/
.flynt-local/
```

These are local workspace metadata and were intentionally not committed.

## Running sessions

There may be a managed web devserver still running:

```text
dx serve --web --port 8080 --addr 127.0.0.1 --open false
```

Native sessions have been relaunched repeatedly with:

```text
cargo run
```

Use native Auspex, not browser mode, for host-local probing. Browser mode is layout smoke only.

## Next implementation slice

Implement stale rehydration for persisted local runtime observation records.

Goal:

```text
On startup, persisted runtime observations are visible but stale/read-only until a fresh probe validates liveness, compatibility, auth, and policy.
```

Concrete tasks:

1. On startup/registry load, mark persisted local observed records as stale unless they were freshly probed in this process.
2. Preserve raw descriptor fields and compatibility evidence as last-known evidence.
3. Do not count stale persisted observations as fresh/live command targets.
4. Add a “Reprobe Runtime” action path for stale local records.
5. Deduplicate by:
   - instance_id
   - state_url
   - startup_url
   - PID only as weak live evidence
6. COP copy should distinguish:
   - fresh observed runtime
   - stale persisted observation
   - needs reprobe

## Known caveats / boundaries

- Internal type names still use `AttachedInstanceRecord`; projection/UI language should continue migrating to observation vocabulary.
- Current policy principal is still a dev principal (`attach_probe_principal()`); future slice should resolve through `styrene-identity` / `styrene-rbac` into `styrene-policy::PrincipalRef`.
- Omegon 0.26 evidence/project-rules capabilities are modeled but not yet read from `.omegon/evidence` or project-rules command output.
- Missing Nex/evidence/project-rules surfaces are advisory degradation, not runtime incompatibility.
- Lifecycle controls must remain unavailable unless runtime ownership is `AuspexOwned` and policy grants approval/signature/audit obligations.

## Recommended first command after context clear

```bash
git status --short
cargo test -p auspex-core --lib -- --nocapture
cargo check
```

Then implement stale rehydration in the instance registry/controller path.
