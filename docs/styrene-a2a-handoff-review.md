# Styrene A2A upstream handoff review

Use this runbook only after the upstream agent provides an immutable commit SHA. It turns the readiness gate into a reproducible acceptance review without modifying `../styrene-rs`.

## Inputs

Record these before running anything:

```text
UPSTREAM_SHA=
UPSTREAM_BRANCH=
A2A_PROFILE_VERSION=
A2A_SDK_VERSION=
IPC_SCHEMA_VERSION=
```

Reject a handoff that provides only a branch name or dirty working tree state. Review an immutable commit.

## 1. Provenance and scope

```bash
cd ../styrene-rs
git cat-file -e "$UPSTREAM_SHA^{commit}"
git show --stat --oneline "$UPSTREAM_SHA"
git diff --check "$UPSTREAM_SHA^" "$UPSTREAM_SHA"
git diff --name-status 779d3fc0.."$UPSTREAM_SHA" -- \
  crates/libs/styrene-a2a \
  crates/libs/styrene-services \
  crates/libs/styrene-ipc \
  crates/libs/styrene-ipc-server \
  crates/apps/styrened \
  docs/a2a-integration-architecture.md \
  docs/a2a-integration-plan.md
```

Confirm that unrelated dirty files are not part of the claimed handoff commit.

## 2. Required API inventory

The handoff must identify concrete paths and public types for:

- envelope profile and profile version;
- official A2A facade/version policy;
- delegation extension and Agent Card negotiation;
- receipt and protocol-error DTOs;
- graph and reconciliation snapshot DTOs;
- `AgentService` operations;
- `DaemonAgents` operations;
- task/event repository interfaces;
- durable production repository implementation;
- protocol-registry handler;
- local IPC operation mappings and capability advertisement.

Capture exported APIs:

```bash
rg -n '^pub (struct|enum|trait|const|fn|mod)|^pub async fn' \
  crates/libs/styrene-a2a/src \
  crates/libs/styrene-services/src \
  crates/libs/styrene-ipc/src
```

Do not treat source presence as proof of integration; verify composition from `styrened` and server dispatch.

## 3. Validation commands

Run the upstream-provided command first, then independently run:

```bash
cargo test -p styrene-a2a
cargo test -p styrene-services
cargo test -p styrene-ipc
cargo test -p styrene-ipc-server
cargo test -p styrened
scripts/validate-a2a.sh
```

If a command is platform-gated or intentionally excluded, require the exact reason and replacement evidence.

## 4. Fixture inventory

Require immutable fixtures or vectors proving:

- canonical envelope JSON/CBOR and protected signing bytes;
- official-SDK message/task/status/artifact/Agent Card round trips;
- accepted, duplicate, conflicting, and rejected receipt behavior;
- root and nested task graph;
- authority-depth attenuation and escalation rejection;
- sequence duplicate, conflict, gap, and restart reconciliation;
- graph projection versus reconciliation snapshot distinction;
- independent cancellation requested/accepted/terminated state;
- unknown optional and required extensions;
- oversized artifact reference;
- old/new Rust and Python IPC peers;
- recognized-invalid A2A traffic does not fall through to chat.

Every copied fixture must retain upstream path, commit SHA, license, profile/schema version, and expected semantic assertion.

## 5. Security review

Reject the handoff if any answer is implicit:

- How does `agent_id` resolve to verification keys?
- Which envelope fields are protected and canonicalized?
- What algorithm/key ID representation is used?
- How are key rotation and revocation handled?
- How is child authority proven to be a subset of parent authority?
- What is the replay-ledger key and retention policy?
- Does authentication occur before deduplication disclosure?
- How do unknown required extensions fail?
- Are transport addresses/topics excluded from signed payloads?
- What prevents recognized invalid A2A data from falling into chat?

## 6. Persistence and recovery review

Verify that production state is durable, not an in-memory demo:

- SQLite migration exists and is composed by `styrened`;
- acceptance transaction atomically records deduplication, envelope/task mutation, graph edge, and event watermark;
- restart recovers committed accepted tasks;
- terminal and dedup retention are configured and tested;
- orphan reconciliation and incomplete sequence gaps survive restart;
- in-memory repositories are restricted to tests.

## 7. Auspex adapter acceptance

Before adding a dependency, create an adapter mapping table using exact upstream types:

| Auspex | Upstream | Lossless? | Evidence retained? |
|---|---|---:|---:|
| `ManagedRunId` | local correlation only | yes | yes |
| `OmegonTaskId` | provider evidence | yes | yes |
| `WorkerId` | agent ID + runtime ID | migration needed | yes |
| parent session/turn | context + parent task | migration needed | must not infer |
| run state | A2A state + delivery/cancel evidence | composite | yes |
| result | message/artifact/reference | migration needed | yes |

Block integration if a conversion silently discards runtime incarnation, ancestry, sequence gaps, required extensions, provider evidence, cancellation confirmation, or artifact references.

## 8. Dependency policy

Acceptable dependency forms:

1. published crate with exact compatible version and lockfile;
2. immutable git revision with `rev = "<full SHA>"`;
3. vendored source with upstream provenance and license.

Do not commit a sibling `path = "../styrene-rs/..."` dependency. Local patches may use `[patch]` outside committed manifests during development, but CI must resolve an immutable source.

## 9. Shadow-mode gate

The first integration remains non-authoritative:

- dispatch occurs once through the existing Omegon path;
- the adapter derives a Styrene/A2A projection from the same evidence;
- no second task is submitted;
- compare root/parent identity, runtime incarnation, task state, cancellation evidence, sequence completeness, and artifacts;
- store divergences as test/audit evidence;
- do not expose speculative A2A UI.

Promotion requires zero unexplained divergence across golden fixtures and at least one live nested-delegation/reconnect/cancellation trace.

## 10. Review result template

```markdown
# Styrene A2A handoff result

- Upstream SHA:
- Profile/SDK/IPC versions:
- Validation: pass/fail
- Required artifacts: complete/incomplete
- Security gates: pass/fail
- Persistence gates: pass/fail
- Compatibility fixtures: pass/fail
- Known deferrals:
- Auspex adapter risks:
- Decision: accept / accept with bounded follow-ups / reject
- First immutable dependency form:
```

A conditional acceptance must name each follow-up, owner, and backend gate. “Tests pass” alone is not sufficient.
