# Session Source Model

## Purpose

Describe the next architectural step after the current controller/session-model split.

The scaffold currently has:
- a UI component
- an app controller
- a host session trait
- a mock host session implementation

The next evolution should make the controller own a session source concept explicitly.

## Proposed next layer

Introduce a session source boundary inside the controller, even if it begins with a single variant.

Example direction:

```rust
enum SessionSource {
    Mock(MockHostSession),
    // Runtime(RealHostSession) later
}
```

## Why this matters

Right now the controller still knows it owns a `MockHostSession` concretely.

That is acceptable for the current scaffold, but it should not become the long-term assumption.

A `SessionSource` layer would make the transition path explicit:
- mock source now
- runtime-backed source later
- potentially attached or embedded host session variants after that

## Rule

Do not add runtime complexity yet just to justify the abstraction. But do make the future substitution path visible before more app behavior hardens around the mock type.
