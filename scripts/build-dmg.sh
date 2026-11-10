#!/usr/bin/env bash
# Build build/Driftwatch_aarch64.dmg: the app plus an Applications link to drag it onto.
set -euo pipefail
cd "$(dirname "$0")/.."

scripts/build-app.sh

dmg=build/Driftwatch_aarch64.dmg
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp -R build/Driftwatch.app "$stage/"
ln -s /Applications "$stage/Applications"

rm -f "$dmg"
hdiutil create -volname Driftwatch -format UDZO -srcfolder "$stage" -ov "$dmg"
echo "Built $dmg"
