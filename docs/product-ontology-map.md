# Product Ontology Map

## Purpose

Map the major Black Meridian products and subsystems into one coherent stack.

This is the practical companion to the stack doctrine.

## Core ontology

| Layer | Statement | Role |
|---|---|---|
| LLMs | how we think | cognition substrate |
| Omegon | how we reason and act | execution / reasoning harness |
| Styrene | who we are and how we communicate | identity / trust / comms layer |
| Auspex | how we see and steer | interface / host / operator shell |

## LLMs

### Role
Cognition substrate.

### Responsibilities
- generation
- synthesis
- interpretation
- latent reasoning

### Non-responsibilities
- identity
- transport
- durable workflow state
- trusted communications

## Omegon

### Role
Reasoning and action harness.

### Responsibilities
- prompt/context assembly
- tool execution
- lifecycle state
- decomposition and orchestration
- verification
- control-plane state for clients

### Non-responsibilities
- root identity
- trust network
- operator UX shell

## Styrene

### Role
Identity and communication substrate.

### Responsibilities
- identity
- trust
- addressing
- transport
- remote communication
- future collaboration substrate

### Non-responsibilities
- agent reasoning engine
- primary workflow harness
- UI shell

## Auspex

### Role
Interface, host, and steering shell.

### Responsibilities
- local operator experience
- remote operator experience
- session hosting
- subsystem supervision
- control surfaces
- state inspection
- phone/desktop projections

### Non-responsibilities
- replacing Omegon’s reasoning engine
- replacing Styrene’s communication/identity layer
- collapsing the stack into a monolith

## Relationship model

### Auspex <-> Omegon
Auspex hosts and presents Omegon.

Omegon remains the engine responsible for reasoning, action, and tool execution.

### Auspex <-> Styrene
Auspex uses Styrene for remote communication, trusted relay, and later multi-device/multi-operator communication.

### Omegon <-> LLMs
Omegon consumes LLMs as cognition backends.

### Styrene <-> Omegon
Styrene can provide trusted connectivity and identity around Omegon sessions, but it should not replace Omegon’s execution responsibilities.

## Product implication

This ontology supports the current direction:
- Auspex can bundle Omegon and Styrene as embedded managed subsystems
- phone clients can connect to desktop Auspex over Styrene
- Omegon remains self-managing as the engine
- LLMs remain swappable cognition backends

## Anti-patterns

### Anti-pattern: the model is the product
Wrong because cognition is only one layer.

### Anti-pattern: Omegon becomes the identity and transport root
Wrong because that is Styrene’s job.

### Anti-pattern: Styrene becomes the reasoning harness
Wrong because that is Omegon’s job.

### Anti-pattern: Auspex becomes a wrapper around accidental backend behavior
Wrong because Auspex should be the intentional operator shell, not a compensating adapter.

## Guiding summary

Black Meridian’s stack should be understood as:
- LLMs provide thought
- Omegon provides agency
- Styrene provides identity and communication
- Auspex provides perception and steering
