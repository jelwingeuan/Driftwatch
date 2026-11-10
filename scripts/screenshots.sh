#!/usr/bin/env bash
# Make the README screenshots (docs/screenshots/timeline-{light,dark}.png) from the
# example data in docs/screenshots/demo-history.json, never from a real scan.
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
cp "$out/demo-history.json" "$app/Contents/Resources/demo-history.json"

# The stub scanner: example history, with the newest scan dated now so the app
# neither starts a scan nor warns that scans are out of date.
cat > "$app/Contents/MacOS/driftwatch-cli" <<'STUB'
#!/bin/bash
now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
case "${1:-}" in
  history) sed "s/\"2026-11-10T18:00:00Z\"/\"$now\"/" "$(dirname "$0")/../Resources/demo-history.json" ;;
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
for theme in light dark; do
  # Per-app appearance flags; the system setting stays as it is.
  if [[ $theme == dark ]]; then
    look=(-AppleInterfaceStyle Dark)
  else
    look=(-NSRequiresAquaSystemAppearance YES)
  fi
  HOME="$tmp/home" "$app/Contents/MacOS/Driftwatch" \
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
  screencapture -x -o -l "$id" "$out/timeline-$theme.png"
  sips -Z 1400 "$out/timeline-$theme.png" >/dev/null
  kill "$pid"
  wait "$pid" 2>/dev/null || true
  pid=""
done
echo "Wrote $out/timeline-light.png and $out/timeline-dark.png"
