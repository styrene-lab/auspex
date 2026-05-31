+++
title = "Styrene RBAC Integration Probe for Auspex"
tags = ["auspex","styrene-rs","rbac","identity","research"]
+++

+++
id = "5a300a96-bcba-47aa-9a74-39ce0a26fd98"
kind = "design_node"

[data]
title = "Styrene RBAC Integration Probe for Auspex"
status = "exploring"
issue_type = "research"
priority = 1
parent = "4d700b75-2425-4182-8fa9-c0ff53b44293"
dependencies = []
open_questions = []
+++

## Overview

# Styrene RBAC Integration Probe for Auspex

---
title: Styrene RBAC Integration Probe for Auspex
status: exploring
tags: [auspex, styrene-rs, rbac, identity, research]
---

# Styrene RBAC Integration Probe for Auspex

Parent: [[authorization-substrate-evaluation-for-auspex]]

## Evidence inspected

Local sibling repository:

```text
../styrene-rs/crates/libs/styrene-rbac
../styrene-rs/crates/libs/styrene-identity
../styrene-rs/crates/apps/styrened
```

## styrene-rbac current model

`styrene-rbac` provides a pure role/capability evaluator:

```rust
RbacPolicy::has_capability(identity_hash, capability) -> bool
RbacPolicy::resolve_role(identity_hash) -> Role
```

Role hierarchy:

```text
Blocked < None < Peer < Monitor < Operator < Admin
```

Capabilities are validated fixed strings. Unknown grants are filtered during normalization/addition.

Important properties:

- Roster entries bind `identity_hash -> role + explicit grants`.
- Blocked hash prefixes override roster entries.
- Static roster takes precedence over hub-signed entries.
- Hub-signed roster entries exist via `SignedRosterEntry` and `TrustedHub`.
- Policy evaluation is pure and side-effect-free.
- `allow_list()` and blocked prefixes are deliberately crate-internal to avoid leaking targeting/evasion data.

## Existing capability shape

Examples relevant to Auspex:

| Existing capability | Possible Auspex use |
|---|---|
| `rpc.status` | local/runtime status read |
| `rpc.inbox_read` | event/session observation |
| `web.read` | read-only web/API view |
| `chat.send` | prompt/chat send |
| `rpc.config_update` | dispatcher/profile/config mutation |
| `terminal.restricted` | restricted terminal HostAction |
| `terminal.full` | full terminal HostAction |
| `rpc.exec` | admin execution |
| `rpc.reboot` | runtime restart/lifecycle |
| `adapter.provision` | package/adapter provisioning |
| `relay.*` | relay/bridge operations |
| `tunnel.*` | tunnel status/establish/teardown |
| `vpn.handshake` | explicit orthogonal VPN grant |

## Gap for Auspex

`styrene-rbac` is identity → capability. Auspex needs:

```text
principal + action + resource + context → decision
```

Context includes:

- target runtime identity
- local ownership (`AuspexOwned`, `UserOwned`, `Unknown`)
- compatibility state
- capability evidence from target runtime
- HostAction class
- package install intent
- approval state
- audit requirement

`styrene-rbac` can resolve the principal's base capabilities, but it does not natively answer resource/context questions such as:

```text
Can this operator stop this specific runtime, given ownership=UserOwned?
Can this operator invoke package.install@1 on runtime X with approval Y?
Can this operator attach to runtime X if runtime X has no signed identity?
```

Those checks would currently be hand-wired around `has_capability()`.

## styrene-identity current model

`styrene-identity` provides canonical public identity:

```text
identity_hash = SHA-256(Ed25519 verifying key)[..16]
```

It also provides:

- deterministic key derivation from a root secret
- `PublicIdentity` hash/pubkey verification
- signed attestations
- identity discovery (`discover()`)
- signer tiers including file signer, keychain, YubiKey, ssh-agent
- hash-only discovery via environment for attribution only

This is a good operator-principal substrate for Auspex.

## Styrened integration points

`styrened` uses `PolicyService::has_capability()` before operations in `DaemonFacade`.

Examples:

- status reads require `rpc.status`
- config mutation requires `rpc.config_update`
- exec requires `rpc.exec`
- reboot requires `rpc.reboot`
- tunnel operations require `tunnel.*`
- roster mutation/admin-level policy changes require `rpc.exec`

This confirms that Styrene's current enforcement style is capability-gated method dispatch, not full resource-aware policy.

## Initial conclusion

Use `styrene-identity` and `styrene-rbac` for principal resolution and baseline capability checks, but do not force Auspex's resource/context semantics into raw capability strings alone.

Auspex needs either:

1. a small resource-aware policy layer over `styrene-rbac`, or
2. a dedicated policy engine, with Styrene identity/RBAC as the principal source.

## Open Questions
