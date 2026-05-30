+++
title = "Auspex 0.25 Compatibility and Capability Registry"
tags = ["auspex","omegon","compatibility","host-actions","nex"]
+++

+++
id = "66fdb8fd-7217-4aa3-bd17-56f76034f930"
kind = "design_node"

[data]
title = "Auspex 0.25 Compatibility and Capability Registry"
status = "exploring"
issue_type = "architecture"
priority = 1
dependencies = []
open_questions = []
+++

## Overview

# Auspex 0.25 Compatibility and Capability Registry

---
title: Auspex 0.25 Compatibility and Capability Registry
status: exploring
tags: [auspex, omegon, compatibility, host-actions, nex, armory]
---

# Auspex 0.25 Compatibility and Capability Registry

## Overview

Audit finding: Auspex currently declares Omegon `0.23.0` compatibility while the active Omegon line is `0.25.4`. Omegon 0.25 introduced orchestration-relevant surfaces: `package.install@1` HostActions, read-only `nex_capability`, and extension initialize metadata surfaced through ACP initialize/session info.

Auspex should add a compatibility/capability registry slice that treats Omegon instances as versioned, policy-bound capability providers.

## Evidence

- Auspex `Cargo.toml` now declares `minimum_version = "0.25.0"`, `maximum_tested_version = "0.25.4"`, `control_plane_schema = 2`.
- Auspex `omegon-compat.toml` declares pre-0.25 Omegon versions unsupported, not degraded.
- Omegon `CHANGELOG.md` 0.25.4 adds `package.install@1` HostAction support and extension initialize metadata for clients such as Flynt.
- Omegon `CHANGELOG.md` 0.25.3 adds `nex_capability` as a read-only resolver.
- Omegon host action implementation validates package install provider/tool/package/scope/privilege policy and derives `nex install --nix <package>` through managed terminal execution.

## Open Questions

- Auspex requires Omegon 0.25.x+; pre-0.25 instances are unsupported rather than degraded.
- Local Omegon source resolves the schema question: `/api/startup` `WebStartupInfo` uses schema `2`, but `OmegonInstanceDescriptor` and `OmegonControlPlane` use `omegon_traits::IPC_PROTOCOL_VERSION`, currently `1`.
- What exact ACP/session-info fields expose extension initialize metadata, and which should become Auspex instance registry fields?
- Should Auspex capability registry persist per-instance capability snapshots, or derive them live on demand?
- What approval model maps existing Auspex operator security tiers to Omegon HostAction approval states?

## Candidate Decisions

- Treat `nex_capability` as read-only evidence only; all mutation must flow through HostAction policy.
- Treat `package.install@1` as a mutating host action requiring operator approval and audit.
- Extend Auspex instance model with Omegon version, control schema, supported HostAction types, extension initialize metadata, and capability evidence.
- Promote Armory/Nex package installation from overlay preflight into desired/actual package-capability reconciliation.

## Implementation Slice

1. [x] Replace the markdown placeholder `omegon-compat.toml` with a real machine-readable compatibility manifest.
2. [x] Add compatibility probe fixtures for supported Omegon 0.25.x and unsupported pre-0.25 instances.
3. [ ] Add an instance capability registry model.
4. [x] Add HostAction policy classification for read-only discovery vs mutating package install.
5. [x] Add audit tests proving unknown/mutating host actions are denied or approval-gated by default.

## Open Questions
