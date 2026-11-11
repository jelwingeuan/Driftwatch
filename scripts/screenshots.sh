#!/usr/bin/env bash
# Make the README screenshots (docs/screenshots/{timeline,overview-*}-{light,dark}.png)
# from the example data in docs/screenshots/demo-history.json, never from a real scan.
#
# It runs a copy of build/Driftwatch.app with its own bundle id (com.driftwatch.demo),
# a temporary HOME and a stub scanner that only prints the example data, then
# captures just that copy's window. Your own Driftwatch settings and history are
# left alone.
set -euo pipefail
cd "$(dirname "$0")/.."

src=build/Driftwatch.app
out=docs/screenshots
demo_id=com.driftwatch.demo
if [[ ! -d $src ]]; then
  echo "No $src. Run scripts/build-app.sh first." >&2
  exit 1
fi

tmp="$(mktemp -d)"
pid=""
cleanup() {
  if [[ -n $pid ]]; then kill "$pid" 2>/dev/null || true; fi
  defaults delete "$demo_id" >/dev/null 2>&1 || true
  rm -rf "$tmp" "$HOME/Library/Preferences/$demo_id.plist" \
    "$HOME/Library/Saved Application State/$demo_id.savedState"
}
trap cleanup EXIT

app="$tmp/Driftwatch.app"
cp -R "$src" "$app"

# The stub scanner: the example history that `demo` wrote to the demo HOME, with
# the newest scan dated now so the app neither starts a scan nor warns that scans
# are out of date.
cat > "$app/Contents/MacOS/driftwatch-cli" <<'STUB'
#!/bin/bash
now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
case "${1:-}" in
  history) sed "s/\"NOW\"/\"$now\"/" "$HOME/demo-history.json" ;;
  scan) echo "{\"snapshot_id\": 42, \"taken_at\": \"$now\", \"baseline\": false, \"changes\": []}" ;;
  *) exit 2 ;;
esac
STUB
chmod +x "$app/Contents/MacOS/driftwatch-cli"

plutil -replace CFBundleIdentifier -string "$demo_id" "$app/Contents/Info.plist"
codesign --force --options runtime -s - "$app/Contents/MacOS/driftwatch-cli"
codesign --force --options runtime -s - "$app"

# Prints the CGWindowID of the given process's normal window, if it has one.
cat > "$tmp/window.swift" <<'SWIFT'
import CoreGraphics
let pid = Int(CommandLine.arguments[1])!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
for w in windows where w[kCGWindowOwnerPID as String] as? Int == pid && w[kCGWindowLayer as String] as? Int == 0 {
    if let b = w[kCGWindowBounds as String] as? [String: Double], b["Height"]! > 200 {
        print(w[kCGWindowNumber as String] as! Int)
        break
    }
}
SWIFT
swiftc -O -o "$tmp/window" "$tmp/window.swift"

mkdir -p "$tmp/home"

# Whole days to move the example scans by, so the newest change is less than a day
# old and the overview's 7- and 30-day windows show the same thing whenever this runs.
days=$(( ($(date -u +%s) - $(date -j -u -f %Y-%m-%dT%H:%M:%SZ 2026-11-10T09:41:00Z +%s)) / 86400 ))

# Writes the example history for one state to the demo HOME: `look` as it is,
# `quiet` with nothing risky, `cantsee` quiet but with privacy permissions unreadable.
demo() {
  local edits=(-e 's/"2026-11-10T18:00:00Z"/"NOW"/')
  for t in $(grep -oE '20[0-9]{2}-[0-9]{2}-[0-9]{2}T[0-9:]{8}Z' "$out/demo-history.json" | sort -u); do
    edits+=(-e "s/$t/$(date -j -u -v+"${days}"d -f %Y-%m-%dT%H:%M:%SZ "$t" +%Y-%m-%dT%H:%M:%SZ)/")
  done
  if [[ $1 != look ]]; then
    edits+=(-e 's/"risk": "(high|medium)"/"risk": "low"/')
  fi
  if [[ $1 == cantsee ]]; then
    edits+=(-e 's|"errors": \[\]|"errors": [{"category": "tcc", "key": "/Library/Application Support/com.apple.TCC/TCC.db", "title": "Couldn’t read privacy permissions", "fields": {"error": "needs Full Disk Access"}, "team_id": null}]|')
  fi
  sed -E "${edits[@]}" "$out/demo-history.json" > "$tmp/home/demo-history.json"
}

# name:state:screen, where screen is the sidebar entry the window opens on.
for shot in timeline:look:all overview-quiet:quiet:overview overview-look:look:overview overview-cantsee:cantsee:overview; do
  IFS=: read -r name state screen <<<"$shot"
  demo "$state"
  for theme in light dark; do
    # Per-app appearance flags; the system setting stays as it is.
    if [[ $theme == dark ]]; then
      look=(-AppleInterfaceStyle Dark)
    else
      look=(-NSRequiresAquaSystemAppearance YES)
    fi
    HOME="$tmp/home" "$app/Contents/MacOS/Driftwatch" -initialScreen "$screen" \
      "${look[@]}" -ApplePersistenceIgnoreState YES >/dev/null 2>&1 &
    pid=$!
    id=""
    for _ in $(seq 40); do
      sleep 0.5
      id="$("$tmp/window" "$pid")"
      [[ -n $id ]] && break
    done
    if [[ -z $id ]]; then
      echo "The demo window didn't open." >&2
      exit 1
    fi
    sleep 2 # let the timeline load and draw
    screencapture -x -o -l "$id" "$out/$name-$theme.png"
    sips -Z 1400 "$out/$name-$theme.png" >/dev/null
    kill "$pid"
    wait "$pid" 2>/dev/null || true
    pid=""
    echo "Wrote $out/$name-$theme.png"
  done
done
