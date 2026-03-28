# Phone Command and Event Surface

## Purpose

Define the initial command and event surface between the phone client and the desktop Auspex host over the Styrene relay path.

This is not a JSON wire spec. It is the semantic surface.

## Phone -> desktop host commands

### Required
- `submit_prompt`
- `cancel_run`
- `request_snapshot`

### Optional early
- `run_slash_command`
- `request_graph`
- `request_session_details`

## Desktop host -> phone events

### Required
- `session_summary`
- `session_state`
- `transcript_chunk`
- `activity_update`
- `system_notice`
- `run_status`
- `compatibility_status`
- `connection_status`

### Optional early
- `tool_detail`
- `thinking_chunk`
- `graph_payload`
- `session_detail`

## Why a relay-specific semantic surface matters

The desktop host should not simply mirror every Omegon event 1:1 to the phone. The phone is consuming a remote session abstraction, not just a raw backend debug stream.

A filtered semantic surface keeps the mobile path simpler and gives the desktop host room to normalize or collapse noisy backend detail.

## Encoding note

The first phone relay protocol should be free to use Styrene-native transport encoding, likely MessagePack over LXMF. These command and event names describe semantic messages, not a requirement that the phone relay mirror the Omegon JSON boundary directly.

## Guiding rule

The first phone protocol should optimize for remote usability, not raw parity with the full local backend stream.
