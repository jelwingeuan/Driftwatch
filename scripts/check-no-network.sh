#!/usr/bin/env bash
# Fail if the app's code uses a network API: Driftwatch makes no network calls.
# File URLs and System Settings links (URL(fileURLWithPath:), NSWorkspace open) are fine.
# Read local files with FileManager or contentsOfFile:, since Data/String(contentsOf:) also fetch URLs.
set -euo pipefail
cd "$(dirname "$0")/.."

pattern='URLSession|URLRequest|NWConnection|NWListener|NWPathMonitor|import Network|CFNetwork|CFSocket|CFStream|WebKit|WKWebView|NSURLConnection|URLSessionWebSocketTask|AsyncImage|socket\(|getaddrinfo|(Data|String)\(contentsOf:'

status=0
grep -rnE "$pattern" app/Sources || status=$?
if [ "$status" -eq 0 ]; then
  echo "Network API in app/Sources (lines above). Driftwatch must not make network calls." >&2
  exit 1
fi
[ "$status" -eq 1 ] || exit "$status" # grep itself failed
echo "No network APIs in app/Sources."
