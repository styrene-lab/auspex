# Black Meridian Stack Doctrine

## Core framing

Black Meridian should think about its stack in four layers:

- **LLMs are how we think**
- **Omegon is how we reason and act**
- **Styrene is who we are and how we communicate**
- **Auspex is how we see and steer**

This is not just branding language. It is an architectural doctrine that defines system boundaries.

## Why this doctrine matters

Without a clear ontology, systems tend to collapse into each other:
- the model becomes the product
- the agent becomes the transport
- the transport becomes identity and execution at once
- the UI becomes a wrapper around backend accidents

This doctrine prevents those category errors.

## Layer 1 — LLMs are how we think

LLMs are the cognition substrate.

They provide:
- synthesis
- generation
- interpretation
- language-space reasoning
- latent conceptual manipulation

They do **not** inherently provide:
- identity
- trust
- durable agency
- workflow structure
- system coordination

That means the model should not be mistaken for the whole product. It is a thinking engine, not the full system.

## Layer 2 — Omegon is how we reason and act

Omegon is the executive harness.

It provides:
- reasoning structure
- context assembly
- lifecycle management
- tool use
- decomposition
- execution
- verification

Omegon operationalizes thought into action.

It is the layer that turns model cognition into:
- decisions
- plans
- edits
- commands
- workflows
- completed work

Omegon should not become the root identity or communications system. It is the acting and reasoning harness.

## Layer 3 — Styrene is who we are and how we communicate

Styrene is the identity and trust layer.

It provides:
- identity
- trust relationships
- addressing
- transport
- communication
- later collaboration and continuity across nodes

Styrene is deeper than networking. It is the layer that answers:
- who is this operator?
- which node is this?
- which peer is trusted?
- how do these entities communicate?

Styrene should not become the reasoning harness. It is the selfhood and communications substrate.

## Layer 4 — Auspex is how we see and steer

Auspex is the interface and perception layer.

It provides:
- operator experience
- visibility
- navigation
- control surfaces
- remote and local interaction
- session hosting and steering

Auspex is how the operator inhabits the system.

It should not become the entire agent runtime. Even when it bundles Omegon and Styrene as embedded managed subsystems, it remains the shell, interface, and steering layer.

## Architectural consequences

### Consequence 1 — LLMs are replaceable cognition backends
Because LLMs are how the system thinks, not who it is, they can be:
- local or remote
- proprietary or open
- upgraded or swapped

without redefining the rest of the stack.

### Consequence 2 — Omegon owns agency
Because Omegon is how the system reasons and acts, it should own:
- tool execution
- lifecycle state
- decomposition
- action verification

It should not be blurred into the transport or identity layer.

### Consequence 3 — Styrene owns trust and communication
Because Styrene is who we are and how we communicate, it is the correct place for:
- remote session trust
- desktop ↔ phone relay
- node identity
- communication channels

It should not be reduced to “just networking.”

### Consequence 4 — Auspex owns product experience
Because Auspex is how we see and steer, it should own:
- first-party session experience
- local host behavior
- remote client UX
- operator-facing control and inspection

It should not have to re-implement Omegon’s reasoning engine or Styrene’s trust substrate.

## Guiding rule

Each layer should be allowed to specialize:
- LLMs think
- Omegon acts
- Styrene identifies and connects
- Auspex interfaces and steers

If a design starts violating that rule, it should be treated as suspect until proven necessary.
