#!/usr/bin/env python3
"""Read canonical managed-agent status from Auspex's private Unix socket."""

from __future__ import annotations

import json
import os
import socket
import sys
import uuid
from typing import Any

PROTOCOL = "auspex.managed-agents.v1"
SCHEMA_VERSION = 1
MAX_FRAME_BYTES = 64 * 1024


def fail(message: str) -> None:
    print(json.dumps({"result": None, "error": message}, separators=(",", ":")))
    raise SystemExit(1)


def require_env(name: str) -> str:
    value = os.environ.get(name, "")
    if not value:
        fail(f"managed-agent bridge unavailable: {name} is not set")
    return value


def read_args() -> dict[str, Any]:
    try:
        value = json.load(sys.stdin)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        fail(f"invalid tool arguments: {error}")
    if not isinstance(value, dict):
        fail("invalid tool arguments: expected an object")
    if set(value) - {"run_id"}:
        fail("invalid tool arguments: unknown property")
    run_id = value.get("run_id")
    if run_id is not None:
        if not isinstance(run_id, str):
            fail("invalid tool arguments: run_id must be a UUID string")
        try:
            uuid.UUID(run_id)
        except ValueError:
            fail("invalid tool arguments: run_id must be a UUID string")
    return value


def receive_frame(connection: socket.socket) -> bytes:
    frame = bytearray()
    while True:
        chunk = connection.recv(min(4096, MAX_FRAME_BYTES + 1 - len(frame)))
        if not chunk:
            fail("managed-agent bridge closed before returning a response")
        newline = chunk.find(b"\n")
        if newline >= 0:
            frame.extend(chunk[:newline])
            break
        frame.extend(chunk)
        if len(frame) > MAX_FRAME_BYTES:
            fail("managed-agent bridge response exceeds frame limit")
    if len(frame) > MAX_FRAME_BYTES:
        fail("managed-agent bridge response exceeds frame limit")
    return bytes(frame)


def main() -> None:
    args = read_args()
    socket_path = require_env("AUSPEX_MANAGED_AGENT_BRIDGE_SOCKET")
    capability = require_env("AUSPEX_MANAGED_AGENT_BRIDGE_CAPABILITY")
    # Presence proves this tool was launched in a bridge-bound primary. Scope remains
    # server-owned; the client never sends a caller-selectable parent identity.
    require_env("AUSPEX_MANAGED_AGENT_PARENT_SESSION_ID")
    request_id = str(uuid.uuid4())
    request = {
        "schema_version": SCHEMA_VERSION,
        "protocol": PROTOCOL,
        "request_id": request_id,
        "capability": capability,
        "operation": {"kind": "agents_status", "run_id": args.get("run_id")},
    }
    encoded = json.dumps(request, separators=(",", ":")).encode() + b"\n"
    if len(encoded) > MAX_FRAME_BYTES:
        fail("managed-agent bridge request exceeds frame limit")

    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.settimeout(4.0)
            connection.connect(socket_path)
            connection.sendall(encoded)
            frame = receive_frame(connection)
    except (OSError, TimeoutError) as error:
        fail(f"managed-agent bridge request failed: {error}")

    try:
        response = json.loads(frame)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        fail(f"managed-agent bridge returned invalid JSON: {error}")
    if not isinstance(response, dict):
        fail("managed-agent bridge returned an invalid response")
    if response.get("schema_version") != SCHEMA_VERSION or response.get("protocol") != PROTOCOL:
        fail("managed-agent bridge returned an unsupported protocol")
    if response.get("request_id") != request_id:
        fail("managed-agent bridge returned a mismatched request ID")
    status = response.get("status")
    if status == "error":
        message = response.get("message")
        fail(message if isinstance(message, str) and message else "managed-agent request rejected")
    if status != "ok" or not isinstance(response.get("runs"), list):
        fail("managed-agent bridge returned an invalid response")
    print(json.dumps({"result": response["runs"], "error": None}, separators=(",", ":")))


if __name__ == "__main__":
    main()
