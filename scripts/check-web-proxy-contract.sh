#!/usr/bin/env bash
set -euo pipefail

ORIGIN="${AUSPEX_WEB_PROXY_ORIGIN:-https://127.0.0.1:9443}"
CURL_INSECURE=()
if [[ "${AUSPEX_WEB_PROXY_INSECURE_CURL:-1}" == "1" ]]; then
  CURL_INSECURE=(-k)
fi

require_http() {
  local label="$1"
  local url="$2"
  local code
  code=$(curl -sS "${CURL_INSECURE[@]}" -o /dev/null -w '%{http_code}' "$url")
  if [[ "$code" != "200" ]]; then
    echo "FAIL $label: expected 200, got $code ($url)" >&2
    exit 1
  fi
  echo "ok $label: $code"
}

require_http "ui" "$ORIGIN/"
require_http "proxy status" "$ORIGIN/_auspex/proxy/status"
require_http "surface snapshot" "$ORIGIN/api/sessions/default/surfaces"

status_json=$(curl -sS "${CURL_INSECURE[@]}" "$ORIGIN/_auspex/proxy/status")
STATUS_JSON="$status_json" python3 - <<'PY'
import json, os
status = json.loads(os.environ["STATUS_JSON"])
assert status["schema_version"] == 1
assert status["mode"] == "proxy-mediated"
assert status["browser_tls"]["public_origin"], "missing browser public_origin"
assert status["identity"]["configured"] is True, "identity must be configured"
assert status["identity"]["strict_daemon_identity"] is True, "daemon strict identity must be active"
assert status["websocket"]["surface_stream_proxy"] is True, "WS stream proxy must be active"
print("ok proxy status contract")
PY

python3 - <<'PY'
import asyncio
import json
import os
import ssl
import sys
import websockets

origin = os.environ.get("AUSPEX_WEB_PROXY_ORIGIN", "https://127.0.0.1:9443")
if origin.startswith("https://"):
    ws_origin = "wss://" + origin[len("https://"):]
    ssl_ctx = ssl._create_unverified_context() if os.environ.get("AUSPEX_WEB_PROXY_INSECURE_CURL", "1") == "1" else None
elif origin.startswith("http://"):
    ws_origin = "ws://" + origin[len("http://"):]
    ssl_ctx = None
else:
    raise SystemExit(f"unsupported origin: {origin}")

async def main():
    async with websockets.connect(f"{ws_origin}/api/sessions/default/surfaces/stream", ssl=ssl_ctx) as ws:
        raw = await asyncio.wait_for(ws.recv(), timeout=5)
        envelope = json.loads(raw)
        assert envelope["schema_version"] == 1
        assert envelope["type"] in {"snapshot", "patch"}
        print("ok websocket surface stream")

asyncio.run(main())
PY
