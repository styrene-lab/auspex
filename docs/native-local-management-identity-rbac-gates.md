+++
title = "Native Local Management Identity and RBAC Gates"
tags = ["auspex","identity","rbac","omegon","security"]
+++

+++
id = "e56e85e7-f632-4c3c-994f-a16c9d713ac0"
kind = "design_node"

[data]
title = "Native Local Management Identity and RBAC Gates"
status = "exploring"
issue_type = "security-design"
priority = 1
parent = "577cd4ab-2324-4e88-bf91-345083d53131"
dependencies = []
open_questions = []
+++

## Overview

# Native Local Management Identity and RBAC Gates

---
title: Native Local Management Identity and RBAC Gates
status: exploring
tags: [auspex, identity, rbac, omegon, security]
---

# Native Local Management Identity and RBAC Gates

Parent: [[native-local-management-mvp-implementation-plan]]

## Problem

Local Omegon discovery is only evidence collection. Once Auspex moves beyond read-only discovery into attach, command, lifecycle, package installation, HostActions, or secret exposure, identity and RBAC become the control boundary.

## Core distinction

Discovery answers:

```text
What local Omegon-like runtimes exist?
```

Authorization answers:

```text
Who is operating Auspex, what runtime identity is targeted, and what action is allowed?
```

PID, port, command line, and startup URL are locators/evidence. They are not principals.

## Identity model

### Operator identity

Auspex needs an operator principal for non-discovery actions:

```rust
OperatorIdentity {
    identity_hash,
    public_key,
    label,
    source,
    can_sign,
}
```

Hash-only identity is attribution-only and must not authorize privileged mutation.

### Runtime identity

Each Omegon runtime should expose stable descriptor identity:

```text
runtime_instance_id
identity_hash / public key
profile
role
capability contract version
```

Auspex must not treat PID or port as runtime identity.

### Ownership

`AuspexOwned` means Auspex has lifecycle evidence for the local OS process. It does not imply the operator may execute tools, install packages, mutate config, expose secrets, or relay sessions.

## Action classes

| Action class | Examples | Required controls |
|---|---|---|
| Read-only local | discover process/port/PID candidates | local-only, non-mutating |
| Read-only runtime | probe startup/state | compatibility check, audit on success |
| Runtime attach | bind candidate to registry/route | operator identity + attach capability |
| Runtime command | prompt, cancel, dispatcher switch | operator identity + command capability + audit |
| Owned lifecycle | launch, stop/restart Auspex-owned PID | ownership proof + lifecycle capability + audit |
| External lifecycle | stop user-owned process | deny by default; admin explicit grant if ever enabled |
| Mutating HostAction | package.install@1, terminal.create@1 | explicit capability + approval + audit |
| Secret exposure | provider/token/vault grants | explicit grant + approval + audit |

## Initial local policy proposal

| Operation | Identity | Capability | Approval | Audit |
|---|---:|---|---:|---:|
| Discover | no | none/local | no | optional |
| Probe | no initially | none/local | no | yes if endpoint responds |
| Attach | yes | `auspex.instance.attach` | no | yes |
| Command | yes | `auspex.instance.command` | maybe | yes |
| Launch | yes | `auspex.instance.launch` | first time | yes |
| Stop owned | yes | `auspex.instance.stop_owned` | yes | yes |
| Restart owned | yes | `auspex.instance.restart_owned` | yes | yes |
| Stop external | yes | `auspex.instance.stop_external` | yes | yes |
| Package install | yes | `auspex.package.install` | yes | yes |
| Secret exposure | yes | `auspex.secret.expose` | yes | yes |

## Acceptance criteria

- [ ] Discovery remains read-only and local-only.
- [ ] Every non-discovery action maps to an explicit capability.
- [ ] Unknown operator identity blocks attach/control/lifecycle.
- [ ] `AuspexOwned` gates lifecycle but does not bypass RBAC.
- [ ] User-owned/external stop is denied by default.
- [ ] Package install and HostActions require approval and audit.
- [ ] PID-file ownership is verified against process/startup evidence before lifecycle actions.

## Open Questions
