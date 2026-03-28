# Session Source Implementation Notes

## Current state

The current scaffold still stores `MockHostSession` directly inside `AppController`, even though the controller and session-model layers now exist.

## Next implementation move

The next concrete code refactor should be:

```rust
enum SessionSource {
    Mock(MockHostSession),
}
```

owned by `AppController`.

The immediate value is not polymorphism for its own sake. The value is making the controller stop depending on the mock session as if it were the permanent runtime source.

## Why this is now justified

Because the scaffold now has:
- controller-owned interaction flow
- session-model trait
- named mock fixtures

there is enough structure in place for a source boundary to be meaningful.

## Constraint

Keep the first `SessionSource` implementation tiny. One variant is enough. Do not invent a fake runtime backend just to satisfy the shape.
