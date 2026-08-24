# Canonical Omegon consumer boundary

## Intent

Move Auspex onto the consumer-facing contracts produced by Omegon's selective kernel decomposition instead of depending on the monolithic source layout or internal runtime authority.

## Scope

Pin shared protocol types and packaged runtime provenance to Omegon revision `227f73502c9c7218ef76ffbb2a020980568c7103`. Remove ordinary-build dependence on a sibling Omegon checkout. Then migrate native attachment to the negotiated IPC snapshot and event contract while retaining HTTP for startup, readiness, and browser compatibility.

Direct use of Omegon's in-process service registry, semantic authority files, projector storage, and private kernel implementation types is excluded.

The migration includes one multiplexed owner for Omegon's single-controller IPC socket, explicit capability gating, bounded reconnect and event buffering, generation-aware state projection, and durable command-outcome semantics. It does not require transport parity for browser builds or expose private decomposition services.

## Success criteria

- Normal Auspex and operator builds consume `omegon-traits` from one immutable revision without a sibling Omegon checkout.
- Release packaging builds the runtime from that same declared revision.
- Native clients negotiate IPC capabilities and reconcile authoritative session state from snapshots and events.
- Prompt and cancellation acceptance is presented as ingress acknowledgement until Omegon reports the resulting queue or session state.
- One native IPC connection owns requests and subscriptions, so command dispatch cannot contend with event streaming for the single controller slot.
- Disconnect and restart handling fences stale instance and session generations without implicitly cancelling Omegon work.
