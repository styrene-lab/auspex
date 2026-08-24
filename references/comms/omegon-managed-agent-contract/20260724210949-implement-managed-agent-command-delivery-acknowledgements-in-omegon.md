+++
id = "04cc3316-9247-4d05-a0a9-0a7c675cf10f"
title = "Implement managed-agent command delivery acknowledgements in Omegon"
tags = []
aliases = []
source_format = "omegon_comm"
source_path = "omegon://omegon-managed-agent-contract"
imported_at = "2026-07-24T21:09:49.799682Z"
imported_reference = true
channel = "omegon-managed-agent-contract"
kind = "agent_communication"

[publication]
enabled = false
visibility = "private"

+++

# Implementation request for ../omegon

Implement the Omegon side of the managed-agent command delivery acknowledgement protocol specified in the Auspex repository at `docs/managed-agent-command-delivery-contract.md` (full contract copied below).

Priority requirements:
1. Add required UUID `command_id` to `delegate_dispatch`, `delegate_get`, `delegate_result`, and `delegate_cancel` request parsing.
2. Emit flat `managed_agent_command_ack` after authenticated parse/validation and durable-in-process handler admission; this ack is delivery/admission only, not operation completion.
3. Add authenticated-session-scoped bounded dedupe keyed by `(session identity, command_id)` with canonical payload SHA-256 digest.
4. Same ID + same digest: replay stored ack, never invoke handler twice. Same ID + different digest: terminal `command_id_conflict`, no handler.
5. Accepted ack must be persisted before or atomically with handler admission so disconnect-before-emission is recoverable by retry.
6. Preserve existing method-specific result envelopes; ack/result order is unconstrained.
7. Add golden fixtures and tests for normal delivery, lost-ack retry/replay, conflicting reuse, overload retryability, and cancellation's separate delivery/result phases.
8. Keep safe rejection text bounded to 1 KiB and never include directives/results/secrets in ack logs or storage.

Coordinate any schema issue back through this channel rather than silently changing the envelope. Commit the Omegon-side implementation and report commit hash plus focused/full validation.

---

Full contract:

See the canonical repository file `docs/managed-agent-command-delivery-contract.md`. Its contents are authoritative. Key wire envelope:

```json
{
  "type": "managed_agent_command_ack",
  "schema_version": 1,
  "command_id": "<uuid>",
  "method": "delegate_get",
  "managed_run_id": "<uuid>",
  "worker_id": "<uuid>",
  "task_id": "delegate_7",
  "status": "accepted",
  "rejection": null,
  "accepted_at_unix_ms": 1750000000000
}
```

Required rejection codes: `unsupported_schema`, `invalid_envelope`, `unauthorized`, `unknown_method`, `command_id_conflict`, `overloaded`, `temporarily_unavailable`. Rejection shape: `{code,safe_message,retryable}`.

Retention: at least max(10 minutes, maximum delegate wall timeout + 60 seconds), bounded LRU/TTL, process-local V1. Dedupe stores only identity, digest, and ack.

The canonical contract includes exact state machine, retry timing, race semantics, security constraints, and acceptance scenarios.
