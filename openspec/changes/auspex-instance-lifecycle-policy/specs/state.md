# state — Delta Spec

## ADDED Requirements

### Requirement: Attached instances track hybrid freshness

Auspex MUST maintain lifecycle freshness for attached Omegon instances using both recorded last-seen evidence from live session activity and control-plane evidence when available.

#### Scenario: Last-seen evidence refreshes on live reconciliation
Given an attached instance record exists in the registry
And a live session snapshot or event confirms that instance is currently attached
When Auspex reconciles runtime state into the state engine
Then the instance lifecycle state is refreshed with a new last-seen observation
And the registry write-back preserves the refreshed observation

#### Scenario: Missing control-plane evidence does not immediately expire a live instance
Given an attached instance was observed recently in the current Auspex session
And a later reconciliation cannot prove control-plane readiness
When the freshness policy is evaluated
Then Auspex marks the instance stale rather than immediately archiving or reaping it
And the operator can still inspect the record in the registry-backed state

### Requirement: Ownership drift is corrected by live evidence

Auspex MUST treat live session/control-plane evidence as authoritative when persisted registry ownership conflicts with current attachment evidence.

#### Scenario: Live session ownership replaces stale persisted owner
Given the registry says an instance belongs to an older session owner
And a current live dispatcher/session snapshot proves the instance belongs to the active session
When Auspex reconciles the instance
Then the registry ownership is rewritten to the live owner
And command routing for that instance uses the live owner context

### Requirement: Cleanup policy differs by instance role

Auspex MUST apply different cleanup behavior for primary dispatcher instances, supervised children, and detached services.

#### Scenario: Session-owned supervised child disappears from the active session set
Given a supervised child instance is registry-owned by the current Auspex session
And the current live session reconciliation no longer reports that child as active
When runtime lifecycle cleanup runs
Then Auspex purges the child from the current session attachment set
And removes the session-owned registry entry unless another policy retains it

#### Scenario: Primary dispatcher disappearance collapses operator routing
Given a primary dispatcher instance is the selected operator route
And live reconciliation no longer reports a host or dispatcher for the active session
When runtime lifecycle cleanup runs
Then Auspex removes the stale session-owned attached routes
And operator routing falls back to the local-shell route

#### Scenario: Detached service remains until policy expiry
Given a detached-service instance is stored in the registry
And it temporarily lacks fresh live evidence
When lifecycle cleanup runs before detached-service expiry
Then Auspex marks the detached service stale, lost, or abandoned according to policy
And does not immediately reap or archive it on first absence

### Requirement: Archive/reap occurs only after lifecycle policy expiry

Auspex MUST separate stale/lost states from terminal archive or reap actions.

#### Scenario: Stale attached instance is retained for operator review before expiry
Given an attached instance has no fresh live evidence
And its role-specific expiry deadline has not elapsed
When Auspex evaluates lifecycle cleanup
Then the instance remains in the registry with a non-ready lifecycle state
And Auspex does not archive or reap it yet

#### Scenario: Expired abandoned detached service becomes reapable
Given a detached-service instance has transitioned to an abandoned state
And its configured expiry window has elapsed
When Auspex evaluates lifecycle cleanup
Then the instance is eligible for reap or archive according to detached-service policy
And the registry reflects the terminal cleanup state
