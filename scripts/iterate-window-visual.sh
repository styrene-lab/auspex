#!/usr/bin/env bash
set -euo pipefail

iterations=${1:-1}
output_dir=${2:-artifacts/visual-iterations}
mkdir -p "$output_dir"

latest=""
for ((iteration = 1; iteration <= iterations; iteration++)); do
  stamp=$(date +%Y%m%d-%H%M%S)
  latest="$output_dir/iteration-${iteration}-${stamp}.png"
  report="${latest%.png}.json"
  scripts/capture-auspex-window.sh "$latest" >/dev/null
  if python3 scripts/analyze-window-capture.py --json "$latest" >"$report"; then
    status=PASS
  else
    status=FAIL
  fi
  python3 - "$iteration" "$status" "$report" <<'PY'
import json, sys
iteration, status, path = sys.argv[1:]
report = json.load(open(path))
print(f"iteration {iteration}: {status}")
for key in (
    "vertical_content_span",
    "largest_vertical_void",
    "horizontal_content_span",
    "horizontal_balance",
    "active_pixel_ratio",
):
    print(f"  {key}: {report[key]}")
failed = [name for name, passed in report["gates"].items() if not passed]
print("  failed gates: " + (", ".join(failed) if failed else "none"))
PY
  [[ "$status" == PASS ]] && break
  (( iteration < iterations )) && sleep 1
done

printf 'latest_capture=%s\n' "$latest"
