# Auspex MVP Path

## Objective

Create the shortest path from design work to an implementation-ready Auspex repo slice.

## Phase 0 — contract and ownership

### Outcomes
- Auspex directory exists in the workspace
- product vision is written down
- Omegon backend contract direction is explicit
- Simple vs Power mode product rule is settled

### Current artifacts
- `README.md`
- `docs/vision.md`
- `docs/control-plane.md`
- `docs/mvp-plan.md`

## Phase 1 — backend hardening in Omegon

### Required before serious UI work
1. Stabilize `/api/state` around a `ControlPlaneStateV1` contract.
2. Include `harness` state in the snapshot.
3. Add `schemaVersion`.
4. Add machine-readable server startup/discovery.
5. Pin/document the WebSocket protocol.
6. Expose Omegon version and control-plane schema identity so Auspex can enforce released-version compatibility.

### Why this comes first
A Dioxus client built against drifting backend internals will accumulate translation debt immediately.

## Phase 2 — Auspex app skeleton

### Deliverables
- Dioxus app shell with shared UI architecture for desktop and mobile
- desktop process manager for launching/attaching Omegon
- backend client for `/api/state`, `/api/graph`, `/ws`
- persistent settings store
- Simple / Power interface mode toggle

### Mobile implication
The initial process-launch story is desktop-specific, but the application architecture should avoid baking desktop assumptions into the transcript, settings, screen model, or backend client. Mobile should be able to attach to a local or nearby Omegon control-plane later without rewriting the app model.

## Phase 3 — Simple mode MVP

### Deliverables
- chat transcript
- composer
- send / cancel
- compact status header
- activity strip / details tray
- actionable warning surfaces

### Success criteria
The default experience feels like a usable local chat client, not an internal telemetry console.

## Phase 4 — Power-user MVP

### Deliverables
- Chat screen oriented around ACP session events, config options, plans, tool calls, and commands
- Graph screen bound to deployed-agent lifecycle topology
- Work screen bound to design/OpenSpec/cleave state
- Session screen bound to `harness`
- desktop-hosted remote session path for phone clients

### Success criteria
The operator can inspect and understand the full harness surface without leaving the app.

## Phase 5 — remote phone client

### Deliverables
- phone client connects to desktop Auspex rather than directly to Omegon
- Styrene-backed transport / comms path
- Simple mode on phone first
- remote cancel / prompt / transcript / compact work state

### Success criteria
A phone operator can act as a trustworthy remote client without the Omegon backend needing to become phone-facing first.

## Phase 6 — post-MVP direction

Potential next steps, not required for v1:
- richer workbench / notes model
- diagramming
- deeper OpenSpec browsing
- context-aware prompting from active selection
- Styrene-backed collaboration
- mobile-local MLX inference

## Repo path principle

Do not build code before the backend seam is stable enough to deserve a client.

The MVP-to-repo path is:
1. write the contract
2. stabilize the backend
3. build the thin desktop shell
4. expand into the broader workspace vision
