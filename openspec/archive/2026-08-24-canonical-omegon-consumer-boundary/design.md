# Design: canonical Omegon consumer boundary

## Authority order

The Rust types in `omegon-traits` at the pinned Omegon revision are authoritative for the native wire contract. Omegon's decided Auspex IPC and launch contracts define transport roles. Internal services, semantic authority JSONL, projections on disk, and implementation modules are not Auspex APIs.

## Dependency and provenance

`auspex-core` links the exact Git revision of `omegon-traits`. The root runtime metadata pins the same repository revision for release packaging. Runtime semver remains an attachment check, while the immutable revision identifies the audited SDK and runtime implementation.

## Transport roles

Native desktop attachment uses project-local MessagePack IPC after HTTP startup and readiness discovery. Browser builds may retain HTTP and WebSocket adapters where Unix sockets are unavailable. Capability tokens from `HelloResponse` gate optional controls; a compiled type does not prove that a runtime advertises the corresponding capability.

Auspex obtains initial truth with `get_state` and tracks session identity, generation, stream, projection frontier, queue depth, active turn, and context revision. `state.reconciled` replaces stale local state after lag. Unknown additive events do not terminate the connection.

`AcceptedResponse` confirms transport ingress only. Queue admission, cancellation, completion, and replacement are rendered from later snapshots or events rather than optimistic local state.

## Connection ownership

Omegon admits one controlling IPC client. Auspex will therefore converge on one background IPC connection owner that performs hello, sends correlated requests, receives responses, and forwards subscribed events. Callers submit typed operations over a bounded in-process channel; they never open competing command connections while the event subscription is active.

The first migration slice may inspect capabilities and seed state through short-lived connections before starting the event subscriber while commands remain on WebSocket. This is transitional and must not be described as native IPC command authority. The command cutover occurs only after the multiplexed owner can correlate responses and events on one socket.

## Capability policy

Auspex records `server_instance_id`, `session_id`, `session_generation`, negotiated protocol, and the exact server capability set returned by hello. `state.snapshot` is required before IPC state can become authoritative. `events.stream` is required before subscribing. Prompt, cancel, slash, model, dispatcher, lifecycle, and shutdown controls are enabled only by their corresponding tokens.

Missing optional capabilities degrade only their owning controls. A missing required native capability keeps the HTTP/WebSocket compatibility adapter active and surfaces the reason. Capabilities are renegotiated after every reconnect and process restart.

## State and generation fencing

The initial `get_state` snapshot is applied before IPC events are presented. Later deltas are accepted only for the active server and session generation. `state.changed` schedules a coalesced snapshot refresh; `state.reconciled` replaces stale state directly on the existing connection. Queue depth and active turn are authoritative for command availability.

The event inbox and request channel are bounded. A lagged server stream is recovered through `state.reconciled`; a locally saturated inbox drops stale deltas, records degradation, and requests a full snapshot rather than growing without bound.

## Failure and fallback

Reconnect uses bounded exponential backoff and repeats hello, identity checks, initial snapshot, and subscription. A changed `server_instance_id` is a process replacement. A changed session generation fences cached session overlays. Disconnect does not issue cancel and does not mark an accepted command complete.

HTTP remains the bootstrap/readiness source. WebSocket remains the browser transport and the temporary desktop command fallback until the single-owner IPC client lands. Fallback is explicit in telemetry and never silently upgrades a WebSocket acknowledgement into durable IPC state.

## Security

Auspex connects only to the startup-advertised project-local socket, relies on Omegon's owner-only socket permissions, enforces the protocol frame limit from `omegon-traits`, and rejects malformed or mismatched response envelopes. Unknown methods, capabilities, and event variants do not grant authority.

## Rollout

Dependency and provenance alignment lands first because it removes false source coupling and makes later protocol work compile against the canonical DTOs. Capability inspection and snapshot-first event attachment land next. A multiplexed single-owner client then enables native command cutover, followed by generation-aware projections and adverse reconnect/lag coverage. Existing HTTP/WebSocket compatibility remains until native IPC behavior and browser separation have dedicated integration coverage.
