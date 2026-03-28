# Session Source Transition Note

## Current implemented state

The controller still owns a concrete `MockHostSession`, but the codebase now has enough layering in place that the next change can be a real `SessionSource` refactor rather than more controller cleanup.

Current layers now are:
- `app.rs` — render + event binding
- `controller.rs` — UI-facing state/actions
- `session_model.rs` — session trait boundary
- `fixtures.rs` — mock session implementation

## Practical conclusion

The next implementation step should now be to introduce:

```rust
enum SessionSource {
    Mock(MockHostSession),
}
```

inside the controller.

That is the point where the app stops assuming that the controller's backing session is permanently the mock implementation.

## Why not keep iterating controller cleanup first?

Because the controller boundary is now good enough for the current scaffold. More micro-refactors there will have diminishing returns until the source boundary becomes explicit.

## Recommendation

The next code change should be `SessionSource`, not another pass at controller accessor churn.
