# telemetry — Delta Spec

## ADDED Requirements

### Requirement: Auspex ingests native Omegon telemetry as sensor data

Auspex MUST treat native Omegon telemetry surfaces as sensor inputs rather than requiring Omegon to emit OpenTelemetry-native export payloads.

#### Scenario: WebSocket turn telemetry is ingested as a provider telemetry sensor
Given Omegon emits a `turn_end` event with `provider_telemetry`
When Auspex receives the event
Then Auspex stores the provider telemetry in its normalized telemetry state
And the stored representation remains tied to the originating session and route context

#### Scenario: Control-plane probes contribute telemetry health state
Given Omegon exposes `/api/startup`, `/api/healthz`, and `/api/readyz`
When Auspex refreshes control-plane probe data
Then Auspex records startup/readiness/health telemetry in its normalized telemetry state
And this telemetry can be correlated with the active instance or route

### Requirement: Auspex owns a normalized internal telemetry model

Auspex MUST normalize sensor inputs into Auspex-owned telemetry state before any export adapter runs.

#### Scenario: Multiple telemetry inputs converge into one session telemetry view
Given Auspex receives websocket events, provider telemetry snapshots, lifecycle registry state, and slash/control results
When Auspex aggregates telemetry for a session
Then the resulting telemetry view contains normalized session, provider, route, and lifecycle telemetry
And UI surfaces read from the normalized view instead of raw sensor payloads directly

### Requirement: OpenTelemetry export is optional and adapter-based

Auspex MUST treat OpenTelemetry/OTLP export as an adapter layered on normalized telemetry state.

#### Scenario: Export adapter maps normalized telemetry without changing sensor ingestion
Given Auspex has normalized telemetry state for sessions and instances
When an OpenTelemetry export adapter is enabled
Then Auspex maps the normalized telemetry into exportable metrics, traces, or logs
And disabling the export adapter does not alter telemetry ingestion or local operator views

### Requirement: Export defaults bound cardinality and redact sensitive data

Auspex MUST avoid high-cardinality or sensitive exports by default.

#### Scenario: Default export excludes raw prompt and transcript payloads
Given telemetry contains prompt text, transcript text, tool args, or filesystem paths
When Auspex prepares default export payloads
Then those raw values are omitted or redacted by default
And exported telemetry uses bounded-cardinality attributes instead

#### Scenario: Provider telemetry exports bounded aggregate fields
Given Auspex has provider telemetry snapshots containing request IDs and quota details
When Auspex prepares default export metrics
Then request IDs are not exported as metric dimensions
And quota/headroom values are exported only through bounded-cardinality aggregate fields
