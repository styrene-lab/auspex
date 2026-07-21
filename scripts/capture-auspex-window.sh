#!/usr/bin/env bash
set -euo pipefail

output=${1:-artifacts/visual-iterations/latest.png}
pid=${AUSPEX_CAPTURE_PID:-$(pgrep -n -x auspex || true)}
mkdir -p "$(dirname "$output")"

if [[ -z "$pid" ]]; then
  echo "no running auspex application process found" >&2
  exit 1
fi

bounds=$(osascript - "$pid" <<'OSA'
on run argv
  set targetPid to item 1 of argv as integer
  tell application "System Events"
    set targetProcess to first application process whose unix id is targetPid
    if (count of windows of targetProcess) is 0 then error "auspex process has no windows"
    set frontmost of targetProcess to true
    set p to position of front window of targetProcess
    set s to size of front window of targetProcess
  end tell
  delay 0.35
  return (item 1 of p as text) & "," & (item 2 of p as text) & "," & (item 1 of s as text) & "," & (item 2 of s as text)
end run
OSA
)

# Select the native application by its exact PID, bring that process forward,
# then capture only its front-window rectangle. Browser/live-view bounds are
# never consulted.
screencapture -x -R"$bounds" "$output"
printf '%s\n' "$output"
