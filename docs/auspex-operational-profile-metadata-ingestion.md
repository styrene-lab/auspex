+++
title = "Auspex Operational Profile Metadata Ingestion"
tags = ["auspex","acp","operational-profile"]
+++

+++
id = "658edd09-9de7-4f19-9c83-b3284bc628c4"
kind = "design_node"

[data]
title = "Auspex Operational Profile Metadata Ingestion"
status = "implementing"
issue_type = "architecture"
priority = 1
dependencies = []
open_questions = []
+++

## Overview

# Auspex Operational Profile Metadata Ingestion

---
title: Auspex Operational Profile Metadata Ingestion
tags: [auspex, acp, operational-profile, implementation]
---

# Auspex Operational Profile Metadata Ingestion

## Design

Auspex needs to ingest ACP initialize/session metadata into its internal `OperationalProfile` model. The source pattern is Flynt-agent's initialize response: identity, required profile, capabilities, policy, and `_meta` are declared at handshake time.

## Accepted input shapes

Initial MVP accepts a JSON metadata object with either:

- top-level `runtime_info`, `capabilities`, and `policy`; or
- top-level `extension_info`, `capabilities`, and `policy`; or
- `_meta.auspex.runtime_info`, `_meta.auspex.capabilities`, and `_meta.auspex.policy`.

This keeps Auspex compatible with both Auspex-specific metadata and Flynt-agent-style extension metadata.

## Mapping

- `runtime_info.name` or `extension_info.name` → `OperationalProfile.name`
- `version` → `OperationalProfile.version`
- `scope` → `OperationalScope`
- `recommended_profile` → `OperationalProfile.recommended_profile`
- `required_profile` → `OperationalProfile.required_profile`
- `capability_contract_version` → `OperationalProfile.capability_contract_version`
- boolean capability fields → `OperationalCapabilities`
- policy fields → `OperationalPolicy`

## Failure policy

Malformed metadata should return `None` rather than inventing an operational profile. Auspex can still operate with compatibility/capability data, but should not claim an operational profile without explicit metadata.

## Implementation Plan

1. Add parser in `operational_profile.rs` from `serde_json::Value`.
2. Unit test Auspex-style `_meta.auspex` metadata.
3. Unit test Flynt-agent-style top-level `extension_info` metadata.
4. Extend `descriptor_ingest` to accept optional metadata and set `observed.operational_profile`.
5. Add controller test proving metadata reaches `fleet_runtime_projection()`.

## Open Questions
