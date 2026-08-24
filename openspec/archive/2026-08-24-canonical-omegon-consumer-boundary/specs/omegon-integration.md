# Omegon integration - Delta Spec

## ADDED Requirements

### Requirement: Immutable canonical contract

Auspex MUST compile shared Omegon protocol types and package the Omegon runtime from the same explicitly declared immutable source revision.

#### Scenario: Ordinary build resolves protocol types
Given no sibling Omegon checkout exists
When an ordinary Auspex or operator build resolves dependencies
Then `omegon-traits` is fetched from the declared immutable Omegon revision
And the build does not require Omegon's monolithic source layout at a sibling path

#### Scenario: Release runtime provenance
Given an Auspex release is packaged
When the Omegon runtime is built
Then the runtime checkout matches the revision declared by Auspex
And the emitted runtime metadata records that revision

### Requirement: Native transport authority

Auspex MUST treat negotiated IPC snapshots and events as the authority for native session state while using HTTP for startup and readiness discovery.

#### Scenario: Native attachment
Given a ready local Omegon instance advertises IPC protocol version 1
When Auspex attaches as a native client
Then Auspex negotiates the runtime's advertised capabilities
And Auspex obtains an initial state snapshot before presenting attached session truth

#### Scenario: Required IPC capability is absent
Given a local Omegon instance does not advertise `state.snapshot` or `events.stream`
When Auspex evaluates native attachment
Then Auspex does not invoke or subscribe to the missing capability
And Auspex reports the compatibility adapter as the active transport

#### Scenario: Single controller connection
Given Omegon permits one controlling IPC client
When Auspex dispatches commands while subscribed to events
Then one multiplexed connection owner sends requests and receives events
And Auspex does not open a competing per-command IPC connection

#### Scenario: Event consumer falls behind
Given Auspex has an active IPC event subscription
When Omegon reports a reconciled state after consumer lag
Then Auspex atomically replaces stale session state with the reconciled snapshot
And Auspex continues the same connection

#### Scenario: Runtime process is replaced
Given Auspex has state for one Omegon server instance identifier
When a reconnect handshake reports a different server instance identifier
Then Auspex fences state associated with the prior process
And Auspex obtains a new initial snapshot before applying later events

#### Scenario: Session generation changes
Given Auspex has cached state for one session generation
When a snapshot reports a newer session generation
Then Auspex replaces generation-scoped queue and active-turn state atomically
And events from the prior generation cannot reactivate stale work

### Requirement: Durable command outcomes

Auspex MUST distinguish transport ingress acknowledgement from authoritative queue, cancellation, and completion state.

#### Scenario: Prompt is accepted by transport
Given Auspex submits a prompt through native IPC
When Omegon returns an accepted response
Then Auspex records only that ingress was acknowledged
And Auspex waits for a queue or session projection before presenting durable admission or completion

#### Scenario: Cancellation is accepted by transport
Given Auspex requests cancellation through native IPC
When Omegon returns an accepted response
Then Auspex does not present the turn as terminal solely from that response
And Auspex derives terminal state from a later authoritative snapshot or event

### Requirement: Bounded native recovery

Auspex MUST bound IPC requests, event buffering, and reconnect attempts while recovering state through authoritative snapshots.

#### Scenario: IPC connection is interrupted
Given Auspex is attached over native IPC
When the socket disconnects without a shutdown acknowledgement
Then Auspex reconnects with bounded exponential backoff
And Auspex does not implicitly cancel or complete the active Omegon turn

#### Scenario: Local event consumer is saturated
Given the bounded Auspex IPC event inbox cannot accept another delta
When a new event arrives
Then Auspex records that reconciliation is required
And Auspex recovers from a full snapshot instead of growing the inbox without bound

### Requirement: Browser transport separation

Auspex MUST keep browser transport behavior separate from native IPC authority.

#### Scenario: Browser build attaches
Given Auspex runs where Unix-domain IPC is unavailable
When it attaches to Omegon
Then it uses the published HTTP or WebSocket compatibility surface
And it does not claim native IPC capability or state authority

### Requirement: Internal kernel state remains encapsulated

Auspex MUST NOT read Omegon semantic authority files, projector storage, or private in-process service implementations as integration contracts.

#### Scenario: New decomposed service appears
Given Omegon publishes a new internal generation-bound service
When Auspex needs behavior derived from that service
Then Auspex consumes an explicitly published IPC or HTTP projection
And Auspex does not link or reach into the private service implementation
