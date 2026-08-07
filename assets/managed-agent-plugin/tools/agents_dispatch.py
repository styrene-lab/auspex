#!/usr/bin/env python3
"""Dispatch a supervised managed agent through Auspex's private bridge."""
import json, os, socket, sys, uuid

PROTOCOL = "auspex.managed-agents.v1"
SCHEMA_VERSION = 1
MAX_FRAME_BYTES = 64 * 1024

def fail(message):
    print(json.dumps({"result": None, "error": message}, separators=(",", ":")))
    raise SystemExit(1)

def env(name):
    value = os.environ.get(name, "")
    if not value: fail(f"managed-agent bridge unavailable: {name} is not set")
    return value

def main():
    try: args = json.load(sys.stdin)
    except Exception as error: fail(f"invalid tool arguments: {error}")
    if not isinstance(args, dict): fail("invalid tool arguments: expected an object")
    allowed = {"operation_id", "directive", "worker_profile", "scope", "supervisor_deadline_seconds"}
    if set(args) - allowed: fail("invalid tool arguments: unknown property")
    operation_id = args.get("operation_id") or str(uuid.uuid4())
    directive = args.get("directive")
    if not isinstance(directive, str) or not directive.strip(): fail("directive is required")
    operation = {
        "kind": "agents_dispatch", "operation_id": operation_id, "directive": directive,
        "worker_profile": args.get("worker_profile", "scout"),
        "scope": args.get("scope", []),
        "supervisor_deadline_seconds": args.get("supervisor_deadline_seconds", 300),
    }
    request_id = str(uuid.uuid4())
    request = {"schema_version": SCHEMA_VERSION, "protocol": PROTOCOL, "request_id": request_id,
               "capability": env("AUSPEX_MANAGED_AGENT_BRIDGE_CAPABILITY"), "operation": operation}
    env("AUSPEX_MANAGED_AGENT_PARENT_SESSION_ID")
    encoded = json.dumps(request, separators=(",", ":")).encode() + b"\n"
    if len(encoded) > MAX_FRAME_BYTES: fail("managed-agent bridge request exceeds frame limit")
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.settimeout(10.0); connection.connect(env("AUSPEX_MANAGED_AGENT_BRIDGE_SOCKET")); connection.sendall(encoded)
            frame = b""
            while b"\n" not in frame and len(frame) <= MAX_FRAME_BYTES: frame += connection.recv(4096)
    except OSError as error: fail(f"managed-agent bridge request failed: {error}")
    try: response = json.loads(frame.split(b"\n", 1)[0])
    except Exception as error: fail(f"managed-agent bridge returned invalid JSON: {error}")
    if response.get("request_id") != request_id: fail("managed-agent bridge returned a mismatched request ID")
    result = response.get("result", {})
    if result.get("status") == "error": fail(result.get("message", "managed dispatch rejected"))
    if result.get("status") != "dispatched": fail("managed-agent bridge returned an invalid response")
    print(json.dumps({"result": result, "error": None}, separators=(",", ":")))

if __name__ == "__main__": main()
