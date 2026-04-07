---
id: auspex-telemetry-aggregation-engine
title: "Auspex telemetry aggregation engine and export adapters"
status: seed
parent: auspex-telemetry-aggregation
tags: [telemetry, aggregator, state-engine, export]
open_questions:
  - "What module boundary should own normalized telemetry state: a controller-adjacent projector, a dedicated telemetry engine, or a registry-backed state subsystem shared across UI surfaces?"
  - "How should lifecycle, provider, control-plane, and delegate/task telemetry be merged into one coherent telemetry snapshot without duplicating session-state logic?"
  - "What adapter interface should OpenTelemetry/OTLP export use so export can be enabled or disabled without affecting ingestion, aggregation, or operator-facing views?"
dependencies: []
related: []
---

# Auspex telemetry aggregation engine and export adapters

## Overview

Extract Auspex telemetry into a dedicated aggregation module that owns normalized telemetry state across provider, control-plane, lifecycle, and task/delegate signals, then layer export adapters (including OTLP) on top without coupling the UI or sensor ingestion paths to exporter concerns.

## Open Questions

- What module boundary should own normalized telemetry state: a controller-adjacent projector, a dedicated telemetry engine, or a registry-backed state subsystem shared across UI surfaces?
- How should lifecycle, provider, control-plane, and delegate/task telemetry be merged into one coherent telemetry snapshot without duplicating session-state logic?
- What adapter interface should OpenTelemetry/OTLP export use so export can be enabled or disabled without affecting ingestion, aggregation, or operator-facing views?
