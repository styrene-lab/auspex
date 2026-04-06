---
id: auspex-embedded-operator-parity
title: "Auspex embedded operator parity via canonical command surface"
status: seed
tags: []
open_questions:
  - "What is the minimal instance-targeted command adapter shape Auspex should adopt now so operator settings/actions can bind to a selected Omegon instance/session without assuming a singleton backend?"
dependencies: []
related: []
---

# Auspex embedded operator parity via canonical command surface

## Overview

Achieve operator-critical parity for embedded Omegon by routing Auspex desktop settings and control surfaces through Omegon's canonical command/slash execution layer, while preserving N+1 instance supervision under a single Auspex authority.

## Research

### Command routing seam for N+1 Omegon instances

Current Auspex command plumbing was bare JSON over a singleton EventStreamHandle. Introduced controller-level TargetedCommand { target: CommandTarget { session_key, dispatcher_instance_id }, command_json } as the first seam away from a global backend assumption. UI still sends over the existing stream, but command production is now instance/session-addressable for later registry-aware routing and canonical slash execution binding.

## Open Questions

- What is the minimal instance-targeted command adapter shape Auspex should adopt now so operator settings/actions can bind to a selected Omegon instance/session without assuming a singleton backend?
