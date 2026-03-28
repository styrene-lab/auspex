# Controller Architecture Notes

## Purpose

Explain the role of `AppController` in the current Auspex scaffold and the direction it should evolve.

## Current state

The controller currently acts as a thin owner of the active host session model.

That is intentional. The point of the controller layer right now is not to contain complex logic; it is to stop the Dioxus component from becoming the application orchestration layer.

## Why the controller exists now

Without the controller, the UI component would directly own:
- scenario switching
n- session replacement
- future runtime binding
- session source selection

That would make later evolution toward real host/runtime integration harder.

## Next likely responsibilities

The controller is the right place for:
- scenario switching rules
- choosing between mock and real session sources
- future runtime attachment/bootstrap actions
- submit orchestration that may later become async or backend-backed
- state transitions that should not live in the UI component

## Rule

The controller should remain thin until real runtime concerns arrive, but the structural boundary is worth introducing early.
