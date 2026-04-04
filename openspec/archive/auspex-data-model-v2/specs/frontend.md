# frontend — Delta Spec

## ADDED Requirements

### Requirement: Structured turn stream
Auspex MUST represent an Omegon session transcript as an ordered stream of turns, not a flat list of chat messages.

Each turn MUST preserve the order of streamed blocks observed from Omegon events.

#### Scenario: Streaming events are grouped into a turn
Given Omegon emits `turn_start`, `thinking_chunk`, `message_chunk`, `tool_start`, `tool_update`, `tool_end`, and `turn_end` events for the same turn
When Auspex ingests the events in order
Then Auspex MUST render a single turn containing the blocks in the same order
And the turn MUST remain associated with its turn number

### Requirement: Thinking blocks are distinct and collapsed by default
Auspex MUST render `thinking_chunk` data as a distinct thinking block rather than merging it into assistant response text.

Thinking blocks MUST be collapsed by default and expandable by the operator.

#### Scenario: Thinking is separated from response text
Given Omegon emits `thinking_chunk` followed by `message_chunk`
When Auspex renders the current turn
Then the thinking content MUST not be merged into the visible assistant response text
And the thinking content MUST be available in a separate block
And that block MUST be collapsed by default

### Requirement: Tool calls render as live cards
Auspex MUST render each Omegon tool invocation as a dedicated tool card.

The card MUST show the tool name and arguments.
The card MUST update live when `tool_update` events arrive.
The card MUST display the final result and failure state when `tool_end` arrives.

#### Scenario: Tool output streams into the same card
Given Omegon emits `tool_start` for a tool call
And Omegon later emits one or more `tool_update` events for the same tool id
And Omegon finally emits `tool_end`
When Auspex renders the transcript
Then the tool call MUST appear as one card
And that card MUST show the tool name and arguments
And the streamed partial output MUST appear in the same card
And the final result MUST replace or append to the same card on completion

### Requirement: Message abort is visible
Auspex MUST handle `message_abort` as an explicit abort of the current in-flight assistant message.

The aborted content MUST remain visible in a muted or struck-through state rather than being silently discarded.

#### Scenario: User cancels a streaming assistant message
Given Omegon is streaming a message and the operator sends `cancel`
When Omegon emits `message_abort`
Then Auspex MUST mark the in-flight message as aborted
And the visible partial content MUST remain in the transcript
And the aborted message MUST be visually distinct from a completed message

### Requirement: Context usage is visible in the shell
Auspex MUST track `context_updated` events and expose the current token usage in the shell header area.

Auspex MUST also surface the same context usage details on the Session screen.

#### Scenario: Context tokens update the header gauge
Given Omegon emits `context_updated` with a token count
When Auspex receives the event
Then the header MUST update its context gauge
And the Session screen MUST display the same token usage value

### Requirement: Session details preserve tool and harness state
Auspex MUST continue rendering harness/session summary data, but it SHOULD expand the session model to include turn-level and tool-level details needed by the live transcript.

#### Scenario: Existing summary data remains available
Given Auspex has already loaded a valid Omegon snapshot
When the operator opens the Session screen
Then the branch, thinking level, provider, memory, cleave, and session stat summaries MUST still be visible
And the richer transcript model MUST not remove those summary fields
