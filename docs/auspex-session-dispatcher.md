---
id: auspex-session-dispatcher
title: "Auspex session dispatcher and operator-facing primary worker"
status: exploring
parent: auspex-multi-agent-runtime
tags: []
open_questions:
  - "What is the exact authority boundary between the session dispatcher and the Auspex supervisor control plane?"
  - "Is dispatcher identity strictly per chat session, or can multiple chats share one dispatcher instance?"
  - "Which profiles/models are eligible for the dispatcher role, and how does operator selection interact with those constraints?"
  - "How should dispatcher identity and concrete model/profile selection be exposed in the session and transcript UI?"
  - "How are delegated child-worker outputs attributed back into the transcript: dispatcher synthesis, child passthrough, or explicit child-labeled blocks?"
  - "Can the dispatcher request detached/background workers directly in the first pass, or only supervised-child workers?"
  - "Which internal automation surfaces are dispatcher-initiated versus Auspex-owned and enforced?"
  - "[assumption] The operator-facing dispatcher should be session-scoped rather than a cross-session global singleton."
dependencies: []
related:
  - auspex-multi-agent-runtime
  - auspex-worker-profiles
  - auspex-worker-inheritance
  - auspex-instance-registry-schema
  - auspex-runtime-backends
---

# Auspex session dispatcher and operator-facing primary worker

## Overview

Define the per-chat-session operator-facing worker that the human speaks to directly. This worker is an Omegon-backed primary-driver instance under Auspex supervision, responsible for direct interaction, delegation decisions, decomposition requests, and session-scoped internal automation orchestration.

This node is intentionally about a **session dispatcher**, not a cross-session global singleton. The phrase "global model" is easy to say, but it hides an important boundary: the dispatcher should remain scoped to the active chat session unless a later design explicitly proves that a cross-session orchestrator is worth the complexity.

## Research

### Existing runtime decisions already support a dispatcher-shaped primary worker

[[auspex-multi-agent-runtime]] already establishes that Auspex is the supervisor/gateway while Omegon workers remain isolated runtimes. It also already defines `primary-driver`, `supervised-child`, and `detached-service` roles. The missing piece is not a new runtime architecture; it is a clearer semantic contract for what the operator-facing `primary-driver` actually does.

### Delegation naturally implies an operator-facing orchestrator

As soon as a visible chat session can decompose work or delegate to child workers, one worker becomes the chooser that the operator is actually interacting with. That chooser is effectively a dispatcher even if the system has not named it yet. Naming the role matters because transcript attribution, profile eligibility, and authority boundaries all depend on it.

### The dispatcher must not collapse the supervisor boundary

The operator-facing dispatcher can decide that delegation or automation is needed, but Auspex should remain the control plane that enforces policy, lifecycle, backend realization, and registry truth. Otherwise the dispatcher turns into an unbounded super-agent, which would violate the accepted supervisor/gateway direction.

## Open Questions

- What is the exact authority boundary between the session dispatcher and the Auspex supervisor control plane?
- Is dispatcher identity strictly per chat session, or can multiple chats share one dispatcher instance?
- Which profiles/models are eligible for the dispatcher role, and how does operator selection interact with those constraints?
- How should dispatcher identity and concrete model/profile selection be exposed in the session and transcript UI?
- How are delegated child-worker outputs attributed back into the transcript: dispatcher synthesis, child passthrough, or explicit child-labeled blocks?
- Can the dispatcher request detached/background workers directly in the first pass, or only supervised-child workers?
- Which internal automation surfaces are dispatcher-initiated versus Auspex-owned and enforced?
- [assumption] The operator-facing dispatcher should be session-scoped rather than a cross-session global singleton.
