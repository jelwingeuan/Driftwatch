#!/usr/bin/env bash
# Build build/Driftwatch.app: the SwiftUI app with the driftwatch CLI inside, ad hoc signed.
set -euo pipefail
cd "$(dirname "$0")/.."

if cargo build --release; then
  cli=target/release/driftwatch
else
  echo "cargo release build failed; using a debug build" >&2
  cargo build
  cli=target/debug/driftwatch
fi

swift build -c release --package-path app
bin="$(swift build -c release --package-path app --show-bin-path)"

app=build/Driftwatch.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp "$bin/Driftwatch" "$app/Contents/MacOS/Driftwatch"
# Not "driftwatch": on a case-insensitive disk that name is the app's own executable.
cp "$cli" "$app/Contents/MacOS/driftwatch-cli"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>com.driftwatch.app</string>
  <key>CFBundleName</key><string>Driftwatch</string>
  <key>CFBundleExecutable</key><string>Driftwatch</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <!-- English UI, so dates read "6 minutes ago", not a mix of English and the system language. -->
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleLocalizations</key><array><string>en</string></array>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep -s - "$app"
echo "Built $app"
