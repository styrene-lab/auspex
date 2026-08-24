## 1. Canonical dependency and provenance
<!-- specs: omegon-integration -->

- [x] Pin `auspex-core` protocol types and root runtime metadata to Omegon revision `227f73502c9c7218ef76ffbb2a020980568c7103`.
- [x] Remove the unused root path dependency and ordinary CI/operator-image sibling Omegon checkout.
- [x] Align runtime compatibility metadata with the canonical `0.29.0-dev` surface.
- [x] Verify release packaging builds and records the pinned decomposition revision.

## 2. Native IPC reconciliation
<!-- specs: omegon-integration -->

- [x] Decode hello identity and capabilities, gate `get_state` and subscription, and seed the controller before starting IPC events.
- [x] Drain IPC events independently of the temporary WebSocket command transport.
- [x] Replace separate command/event connections with one bounded multiplexed IPC connection owner.
- [x] Make the multiplexed IPC owner the native desktop command and event transport after HTTP startup/readiness discovery.
- [x] Project server identity, session generation, stream, frontier, queue, active-turn, and context revision fields.
- [x] Coalesce `state.changed` refreshes and apply `state.reconciled` snapshots on the active connection.
- [x] Fence stale process/session generations and renegotiate capabilities after reconnect.
- [x] Bound request queues, event buffering, response deadlines, and reconnect backoff.

## 3. Command outcome semantics
<!-- specs: omegon-integration -->

- [x] Preserve command correlation identifiers across IPC control requests.
- [x] Render prompt and cancellation acceptance as ingress acknowledgement until authoritative state follows.
- [x] Add real-runtime coverage for queued prompts, cancellation, replacement, lag reconciliation, and disconnect without implicit cancellation.

## 4. Transport separation and observability
<!-- specs: omegon-integration -->

- [x] Keep browser HTTP/WebSocket attachment independent of native IPC-only state.
- [x] Report negotiated protocol, server identity, capability set, active adapter, fallback reason, and reconciliation health.
- [x] Remove the temporary desktop WebSocket command fallback after multiplexed IPC adverse-path coverage passes.
