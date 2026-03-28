# Phone Simple Mode Projection

## Purpose

Pin the minimum state the phone Simple mode actually needs from the desktop host.

This keeps the first remote implementation from over-exposing backend detail.

## Minimum Simple mode payload

### Required blocks
- `session_summary`
- `transcript`
- `activity`
- `work`
- `connection`
- `compatibility`

## Expected user capabilities

With only this payload, the phone user should be able to:
- see the conversation
- send a prompt
- cancel a run
- understand whether the system is connected
- understand what the system is currently doing
- understand compact current work context

## Compact work projection

Simple mode does not need full lifecycle inventory.

It needs at most:
- current focused work item
- compact implementation progress indicator
- compact parallel-task indicator when active
- compact OpenSpec progress when relevant

## Activity projection

Activity should prefer a summarized state like:
- `reading files`
- `running tests`
- `waiting for model`
- `running 3 parallel tasks`
- `reconnecting to host`

not raw tool/event names unless the operator asks for details.

## Guiding rule

If the phone Simple mode cannot justify a field in terms of direct user value, that field should remain on the desktop host side for now.
