---
id: auspex-primary-coordinator
title: "Auspex primary coordinator posture and fleet-control plan"
status: planning
parent: auspex-multi-agent-runtime
tags: []
open_questions:
  - "Which control actions require operator confirmation versus policy-only approval?"
  - "Should workflow scheduling live inside the primary coordinator, an Auspex scheduler, or a separate queue controller?"
  - "How should Flynt board tasks, Sentry queue items, and omegon-flow runs deduplicate equivalent work?"
dependencies:
  - auspex-multi-agent-runtime
  - auspex-runtime-backends
  - auspex-worker-inheritance
  - auspex-session-dispatcher
related:
  - auspex-desktop-shell-frame
  - auspex-worker-profiles
  - auspex-cnpg-persistence
---

# Auspex primary coordinator posture and fleet-control plan

## Overview

The primary integrated agent should be treated as the **operator-facing coordinator for an agent fleet**, not as a coding agent with optional helpers.

It may execute small work inline, but its identity is the control-plane intelligence that watches work sources, chooses execution lanes, deploys or binds agents, reconciles workflow state, and publishes structured operating state into Auspex.

The current `primary-driver` role remains the transport/runtime role for compatibility.
The product-facing posture should become **Primary Coordinator**.

## Problem

A single-chat mental model fails once Auspex supervises:

- multiple deployed Omegon agents
- independent workflow runs
- Sentry/task-queue intake
- Flynt board/task handoffs
- Kubernetes-backed workers
- detached background services
- policy-bound deployment operations
- agent-to-agent escalation

If the primary agent believes its job is "perform this coding task," it will over-focus on inline execution, under-report background work, under-use fleet capacity, and blur who has authority to deploy, assign, verify, pause, or retire agents.

## Decision

### Primary Coordinator is the product posture for the operator-facing primary-driver worker

**Status:** proposed

`primary-driver` remains the low-level role identifier until the control-plane schema migrates.
Auspex UI, profile copy, and operator-facing documentation should use **Primary Coordinator** when referring to the integrated agent's behavior.

### The coordinator manages work, not just conversations

**Status:** proposed

The coordinator should ingest work from:

- operator directives
- workflow triggers
- scheduled jobs
- Sentry/task queues
- Flynt board items
- deployment health events
- external webhooks
- other agents requesting escalation
- policy/config drift detectors

Each source produces a normalized work item or coordination event before execution.

### ExecutionLane is the central unit of independent work

**Status:** proposed

An `ExecutionLane` represents one independently progressing stream of work regardless of origin.
It may be backed by one prompt, one workflow run, one queue item, one deployment operation, or several delegated assignments.

This prevents the UI and runtime from assuming that "chat turn" equals "work item."

### Deployment is a coordinator capability behind policy gates

**Status:** proposed

The coordinator needs tools to deploy, scale, restart, retire, and bind agents/workflows.
Those tools must pass through Auspex policy enforcement rather than being unmediated shell actions.

High-impact operations must emit audit records and usually require explicit operator approval.

## First-order primitives

### WorkSource

Where work originated.

Examples:

- `operator`
- `workflow`
- `sentry`
- `flynt`
- `schedule`
- `webhook`
- `agent_escalation`
- `deployment_health`
- `policy_drift`

### WorkItem

Normalized objective with:

- id
- source
- title / objective
- priority
- due/deadline hints
- policy domain
- required capabilities
- workspace binding
- workflow/task references
- current state

### ExecutionLane

Independent workstream with:

- lane id
- source work item ids
- active assignments
- current coordinator decision
- blockers
- verification state
- COP projection state
- audit correlation id

### WorkAssignment

Binding from a work item or lane segment to an agent/runtime:

- assigned agent id
- required capability snapshot
- propagation object
- expected outputs
- verification criteria
- timeout/retry policy

### AgentCapability

Resolved ability of a specific agent:

- tools
- skills/armory packages
- auth references
- workspace locality
- model tier
- thinking budget
- network/runtime access
- current load
- trust posture

### DeploymentIntent

Policy-checked desired deployment change:

- deploy agent
- scale agent
- restart agent
- retire agent
- install skill/armory package
- bind workflow
- attach secret reference
- change runtime placement

### CoordinationEvent

Append-only event stream:

- work accepted
- lane opened
- agent assigned
- agent deployed
- workflow started
- handoff created
- blocked
- escalated
- verified
- superseded
- cancelled
- retired

### CopProjection

Structured operator view emitted from coordinator/runtime state:

- fleet status
- active lanes
- blocked lanes
- workflow run status
- deployment health
- queue pressure
- policy approvals
- recent events

## Tool surface implications

The coordinator needs read tools before write tools.

### Phase 1: observe

- `fleet.list`
- `fleet.inspect_agent`
- `fleet.capabilities`
- `workflow.list`
- `workflow.inspect_run`
- `queue.list_sources`
- `queue.peek`
- `lane.list`
- `lane.inspect`
- `policy.explain`

### Phase 2: decide and stage

- `lane.open`
- `lane.plan`
- `assignment.propose`
- `deployment.plan`
- `workflow.plan_start`
- `policy.request_approval`

### Phase 3: act

- `agent.deploy`
- `agent.restart`
- `agent.retire`
- `agent.install_skill`
- `workflow.start`
- `workflow.pause`
- `workflow.resume`
- `workflow.cancel`
- `assignment.create`
- `assignment.reassign`
- `lane.cancel`

### Phase 4: verify and close

- `assignment.verify`
- `lane.verify`
- `lane.close`
- `cop.publish`
- `audit.append`

## UI implications

### COP

COP becomes the default operational dashboard.

It should prioritize:

- active execution lanes
- blocked/escalated work
- fleet capacity and degraded agents
- workflow runs requiring attention
- queue pressure
- deployment health
- pending approvals

Second-order implication: COP cannot be a passive status collage.
It must become a command surface where selecting a lane, agent, queue, or approval updates the right rail and scopes Chat/Graph/Workflow.

### Chat / Direct Line

Chat is the direct line to the Primary Coordinator.

It should show:

- posture: `primary coordinator`
- authority scope
- selected target context
- policy constraints
- available deployment/workflow controls
- active lane context if one is selected

It should not imply that the transcript is the system of record.
The transcript is a command/control channel; execution state belongs to lanes, assignments, workflows, COP, and audit.

Second-order implication: prompt suggestions should be state-aware:

- deploy a worker for this lane
- assign this blocked workflow step
- pull next queue item
- pause noncritical lanes
- inspect agent capability mismatch

### Graph

Graph becomes the live topology of deployed work:

- agents
- runtimes/clusters
- workflows
- execution lanes
- assignments
- handoff/escalation edges
- dependency edges

It should not become a task board.
It answers "what is connected to what, and why is work moving that way?"

Second-order implication: graph nodes need stable ids and event-backed freshness, otherwise topology will lie under churn.

### Workflow

Workflow remains definition and run inspection/control:

- edit/import `omegon-flow` definitions
- start runs
- inspect step state
- bind steps to required capabilities
- retry/pause/cancel runs
- promote workflow outputs into work items or COP alerts

Second-order implication: workflow runs and execution lanes are related but not identical.
A single workflow run may create multiple lanes; a lane may coordinate several workflow runs.

### Left rail

The left rail should be organized around control-plane scope:

- Primary Coordinator
- Deployed Agents
- Active Lanes
- Workflow Runs
- Queues / Sources

Second-order implication: selecting "Primary Coordinator" should not hide fleet work.
It should show the coordinator's current view of the fleet.

### Right rail

The right rail becomes scoped inspection and action:

- selected agent capability/deployment/policy
- selected lane plan/assignments/blockers
- selected workflow run steps
- selected queue item metadata
- selected approval request

Third-order implication: action buttons in the right rail must be policy-aware.
Disabled controls should explain whether the blocker is capability, identity, approval, runtime, or stale state.

### Audit

Audit becomes a first-class safety surface.

It must record:

- who/what opened a lane
- who/what assigned work
- which agent deployed or changed state
- policy basis for each action
- secrets/capability references used
- verification results
- operator approvals
- rollbacks/cancellations

Third-order implication: audit correlation ids should span COP, Chat, Graph, Workflow, and backend events.

## Behavioral posture

The Primary Coordinator profile should include:

- You are the operator-facing coordinator for an agent fleet.
- You manage work from prompts, workflows, queues, alerts, schedules, and agent escalations.
- You maintain global operating state before acting locally.
- You may execute inline only when the work is small, urgent, or requires your direct authority.
- Prefer deploying or assigning suitable agents for separable work.
- Treat deployment, workflow, policy, identity, and secret changes as high-impact operations.
- Publish structured state to COP whenever the operator needs operational awareness.
- Record coordination events and audit-sensitive decisions.
- Explain why a work item was assigned to a given agent or held for approval.
- Ask the operator before destructive, authority-expanding, or ambiguous actions.

## Second- and third-order effects

### State explosion

Multiple work sources and agents will create many more states than a chat transcript can explain.
Auspex needs normalized state machines for work items, lanes, assignments, workflows, and deployments.

Mitigation:

- make `ExecutionLane` the operator-facing unit of concurrency
- project lane summaries into COP
- retain full detail in right rail and audit

### Duplicate work

The same objective may arrive from Flynt, Sentry, workflow triggers, and operator prompts.

Mitigation:

- work items need source references and dedupe keys
- coordinator should merge or supersede equivalent work
- UI should show supersession, not silently discard duplicates

### Authority confusion

If the coordinator can deploy agents and change workflows, it can alter the control plane it is using.

Mitigation:

- policy engine must gate write/deploy actions
- privileged actions need audit and likely operator confirmation
- authority scope must be visible in Chat and right rail

### Cost and capacity drift

Autonomous deployment can accidentally scale expensive models or long-running workers.

Mitigation:

- profile budgets
- per-lane cost/runtime bounds
- global fleet capacity limits
- visible queue pressure and active worker count

### Security boundary creep

Skill installation, secret attachment, and workflow binding can silently widen what an agent can do.

Mitigation:

- capability references, not raw secret copies
- explicit inheritance objects
- policy-diff previews before deployment
- audit records for capability expansion

### UI overload

Every lane, agent, workflow, and queue can demand attention.

Mitigation:

- COP ranks attention, not raw volume
- Graph is topology, not notification feed
- right rail scopes detail to the current selection
- Chat suggestions are contextual and sparse

### Reconciliation failure

Kubernetes, local processes, and detached services can diverge from desired state.

Mitigation:

- desired vs observed state is explicit
- stale/lost workers remain visible until reconciled or reaped
- coordinator must explain reattach and cleanup decisions

## Implementation sequence

1. Rename operator-facing UI posture from Primary Driver to Primary Coordinator.
2. Add coordinator profile language to Omegon/Auspex configuration docs.
3. Introduce read-only structs for `WorkSource`, `WorkItem`, `ExecutionLane`, `WorkAssignment`, and `CoordinationEvent`.
4. Project mock/derived execution lanes into COP.
5. Add right-rail inspection for selected lane/agent/workflow.
6. Add read-only fleet/workflow/queue tools to the coordinator.
7. Add policy-gated staging commands for deployment and assignment.
8. Add actuation commands for agent deployment, workflow control, and reassignment.
9. Wire Graph to agents, lanes, workflow runs, assignments, and deployment topology.
10. Add audit correlation ids across Chat, COP, Graph, Workflow, and backend events.

## Near-term UI copy changes

Use **Primary Coordinator** in operator-facing UI.
Keep `primary_driver` in machine-readable descriptors until schema migration.

Preferred labels:

- `Primary Coordinator`
- `fleet coordinator`
- `direct line`
- `execution lanes`
- `fleet`
- `workflow runs`
- `queues`
- `policy gates`

Avoid labels that imply a single coding session:

- `Local Agent`
- `Primary Driver` in visible UI
- `chat session` as the system of record
- `tasking` for fleet/workflow state
