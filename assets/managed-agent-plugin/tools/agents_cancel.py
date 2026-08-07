#!/usr/bin/env python3
"""Cancel an Auspex-supervised managed run through the private bridge."""
import json, os, socket, sys, uuid
PROTOCOL="auspex.managed-agents.v1"; SCHEMA_VERSION=1; MAX_FRAME_BYTES=65536

def fail(message):
    print(json.dumps({"result":None,"error":message},separators=(",",":"))); raise SystemExit(1)
def env(name):
    value=os.environ.get(name,"")
    if not value: fail(f"managed-agent bridge unavailable: {name} is not set")
    return value

def main():
    try: args=json.load(sys.stdin)
    except Exception as error: fail(f"invalid tool arguments: {error}")
    if not isinstance(args,dict) or set(args)-{"operation_id","run_id","reason"}: fail("invalid tool arguments")
    try: run_id=str(uuid.UUID(args.get("run_id","")))
    except ValueError: fail("run_id must be a UUID string")
    operation={"kind":"agents_cancel","operation_id":args.get("operation_id") or str(uuid.uuid4()),"run_id":run_id,"reason":args.get("reason")}
    request_id=str(uuid.uuid4()); env("AUSPEX_MANAGED_AGENT_PARENT_SESSION_ID")
    request={"schema_version":SCHEMA_VERSION,"protocol":PROTOCOL,"request_id":request_id,"capability":env("AUSPEX_MANAGED_AGENT_BRIDGE_CAPABILITY"),"operation":operation}
    encoded=json.dumps(request,separators=(",",":")).encode()+b"\n"
    try:
        with socket.socket(socket.AF_UNIX,socket.SOCK_STREAM) as connection:
            connection.settimeout(10); connection.connect(env("AUSPEX_MANAGED_AGENT_BRIDGE_SOCKET")); connection.sendall(encoded)
            frame=b""
            while b"\n" not in frame and len(frame)<=MAX_FRAME_BYTES: frame+=connection.recv(4096)
    except OSError as error: fail(f"managed-agent bridge request failed: {error}")
    try: response=json.loads(frame.split(b"\n",1)[0])
    except Exception as error: fail(f"managed-agent bridge returned invalid JSON: {error}")
    result=response.get("result",{})
    if response.get("request_id")!=request_id or result.get("status")!="dispatched": fail(result.get("message","managed cancellation rejected"))
    print(json.dumps({"result":result,"error":None},separators=(",",":")))
if __name__=="__main__": main()
