---
id: auspex-telemetry-aggregation
title: "Auspex telemetry aggregation and OpenTelemetry export"
status: resolved
parent: auspex-multi-agent-runtime
tags: [telemetry, opentelemetry, aggregator, observability]
open_questions:
  - "What native Omegon telemetry signals should Auspex treat as first-class sensor inputs: websocket AgentEvents, provider telemetry snapshots, startup/health/readiness probes, lifecycle registry state, and slash/control results?"
  - "What normalized telemetry model should Auspex own before any OTLP export: session telemetry, provider quota/headroom telemetry, route/dispatcher telemetry, instance lifecycle telemetry, and task/delegate telemetry?"
  - "Which telemetry should remain local/operator-facing only, and which should be exportable to OpenTelemetry backends with bounded cardinality and redaction?"
  - "Should Auspex own OTLP export directly, or should it first build a stable internal telemetry bus and add export adapters afterwards?"
dependencies: []
related: []
---

# Auspex telemetry aggregation and OpenTelemetry export

## Overview

Define Auspex as the telemetry aggregator over Omegon sensor data: ingest native event/control-plane/provider telemetry, normalize it into Auspex-owned telemetry state, and optionally export to OpenTelemetry/OTLP-compatible systems without making Omegon itself OTel-native.

## Research

### Telemetry contract assessment against Omegon rc.21

Verified against Omegon rc.21 code: native telemetry surfaces include websocket event projection with `turn_end` carrying `provider_telemetry`, typed `ProviderTelemetrySnapshot` in `omegon-traits`, and `/api/startup`, `/api/healthz`, `/api/readyz` control-plane probes. No concrete OTLP/OpenTelemetry exporter surface was found in Rust runtime code; Omegon's own `docs/opentelemetry-fit-assessment.md` explicitly recommends keeping harness-native telemetry as the source model and adding optional export adapters later. This confirms the boundary: Omegon emits sensor data, Auspex aggregates and optionally exports.

## Decisions

### Treat Omegon as a sensor producer, not an OpenTelemetry exporter

**Status:** accepted

**Rationale:** Omegon rc.21 exposes rich native telemetry surfaces — websocket AgentEvents, provider telemetry snapshots, and startup/health/readiness probes — but not a first-class OTLP/Prometheus export contract. Auspex should ingest these native signals as sensor inputs rather than forcing Omegon to own OTel concerns.

### Auspex owns a normalized telemetry model before any external export

**Status:** accepted

**Rationale:** The same native Omegon signals must drive local operator UX, historical session/state aggregation, and eventual external export. Auspex therefore needs a stable internal telemetry model spanning session telemetry, provider quota/headroom telemetry, route/dispatcher telemetry, instance lifecycle telemetry, and task/delegate telemetry before adding OTLP export adapters.

### OpenTelemetry export is an adapter layered on Auspex telemetry state

**Status:** accepted

**Rationale:** If Auspex exports telemetry to OTLP-compatible backends, the exporter should map from Auspex-owned normalized telemetry into low-cardinality traces/metrics/logs. This avoids coupling operator UX semantics or sensor vocabulary directly to OTel semantic conventions.

### Export only bounded-cardinality and redacted telemetry by default

**Status:** accepted

**Rationale:** Prompts, tool args, raw transcript text, filesystem paths, branch names, child labels, and provider request IDs can leak sensitive data or explode cardinality. Auspex should keep detailed sensor payloads local by default and export only bounded-cardinality aggregates or explicitly redacted forms unless the operator opts into richer export.

## Open Questions

- What native Omegon telemetry signals should Auspex treat as first-class sensor inputs: websocket AgentEvents, provider telemetry snapshots, startup/health/readiness probes, lifecycle registry state, and slash/control results?
- What normalized telemetry model should Auspex own before any OTLP export: session telemetry, provider quota/headroom telemetry, route/dispatcher telemetry, instance lifecycle telemetry, and task/delegate telemetry?
- Which telemetry should remain local/operator-facing only, and which should be exportable to OpenTelemetry backends with bounded cardinality and redaction?
- Should Auspex own OTLP export directly, or should it first build a stable internal telemetry bus and add export adapters afterwards?
