+++
title = "Auspex ACP Operational Profile Adoption"
tags = ["auspex","acp","flynt-agent","profiles"]
+++

+++
id = "982c335e-bad2-42a7-a572-afc8f3fd54e5"
kind = "design_node"

[data]
title = "Auspex ACP Operational Profile Adoption"
status = "exploring"
issue_type = "architecture"
priority = 1
dependencies = []
open_questions = []
+++

## Overview

# Auspex ACP Operational Profile Adoption

---
title: Auspex ACP Operational Profile Adoption
status: exploring
tags: [auspex, acp, flynt-agent, profiles, orchestration]
---

# Auspex ACP Operational Profile Adoption

## Overview

Flynt-agent demonstrates the clean ACP adoption pattern Auspex should use for operational profiles: make runtime identity, required profile, capabilities, and policy machine-readable during `initialize`, then let clients/orchestrators consume that metadata instead of inferring behavior from tool names or repo layout.

## Evidence

- `flynt-agent` is a native Omegon extension owned by the Flynt product repo, tracked externally by `omegon-extensions`.
- `crates/flynt-agent/manifest.toml` declares native binary `flynt-agent`, startup `ping_method = "get_tools"`, SDK version metadata, and a `mind` profile for project structure/recent documents/working context.
- `flynt-agent` `initialize` returns:
  - `protocol_version = 2`
  - `extension_info` with name/version/sdk/runtime minimum/scope/project root/profile versions
  - `recommended_profile` and `required_profile` set to `flynt-agent`
  - capability flags
  - policy metadata
  - `_meta.flynt` mirrored metadata
  - tools embedded in the initialize result
- The policy explicitly declares project memory scope, cross-pollination forbidden, and requirements for UI-state/surface-guide evidence before claims.

## Corrected Framing

Flynt is not the authority for Auspex's operational cockpit. Flynt-agent is an extension adoption pattern: it shows how a product-owned workspace capability cleanly presents itself to Omegon/ACP.

Auspex should apply the same pattern to operational orchestration profiles, while retaining ownership of fleet semantics: instance state, supervision, dispatch, compatibility, HostAction policy, audit, and capability reconciliation.

## Open Questions

- [assumption] Auspex operational profiles should be exposed through ACP-compatible initialize/session metadata even when Auspex is supervising full Omegon instances rather than ordinary extensions.
- [assumption] A profile name like `auspex-orchestrator` should become the required/recommended profile for orchestration-capable instances.
- Which fields belong in generic `extension_info`/runtime info versus `_meta.auspex`?
- How should Auspex distinguish project-scoped, fleet-scoped, and host-scoped memory/state?
- What policy fields are mandatory before Auspex can dispatch work to worker Omegon instances?

## Candidate Auspex Profile Metadata

```json
{
  "protocol_version": 2,
  "runtime_info": {
    "name": "auspex-orchestrator",
    "version": "...",
    "omegon_min_version": "0.25.4",
    "control_plane_schema": 2,
    "scope": "fleet",
    "recommended_profile": "auspex-orchestrator",
    "required_profile": "auspex-orchestrator",
    "capability_contract_version": 1
  },
  "capabilities": {
    "instance_registry": true,
    "dispatch": true,
    "supervision": true,
    "host_actions": true,
    "package_reconciliation": true,
    "audit": true
  },
  "policy": {
    "host_action_mutation_requires_approval": true,
    "unknown_host_actions": "deny",
    "capability_discovery": "read_only",
    "dispatch_requires_compatible_instance": true,
    "cross_project_state": "explicit_grant_only"
  }
}
```

## Implementation Direction

1. Define Auspex operational profile metadata schema.
2. Add runtime/session compatibility handshake that captures Omegon version, schema, required profile, capabilities, and policy.
3. Treat Flynt-agent ACP initialize as the compatibility pattern, not as a UI ownership model.
4. Bind capability registry and HostAction policy to profile metadata rather than hard-coded assumptions.

## Open Questions
