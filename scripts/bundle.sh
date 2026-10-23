#!/usr/bin/env bash
# Builds target/Dislexia.app from a release binary. Run after
# `cargo build --release`. Ad-hoc signed for local use.
set -euo pipefail
cd "$(dirname "$0")/.."

APP=Dislexia
VERSION=$(python3 - <<'PY'
import re
print(re.search(r'^version = "(.*)"', open("Cargo.toml").read(), re.M).group(1))
PY
)
BUILD=target/release/dislexia
BUNDLE="target/${APP}.app"

[ -f "$BUILD" ] || { echo "missing $BUILD — run cargo build --release first" >&2; exit 1; }

# The bundle needs the icon; regenerate it when out of date.
if [ ! -f assets/icon.icns ] || [ media/icon-typewriter.svg -nt assets/icon.icns ]; then
    scripts/make-icons.sh
fi

rm -rf "$BUNDLE"
mkdir -p "${BUNDLE}/Contents/MacOS" "${BUNDLE}/Contents/Resources"
cp "$BUILD" "${BUNDLE}/Contents/MacOS/${APP}"
cp assets/icon.icns "${BUNDLE}/Contents/Resources/"
cat > "${BUNDLE}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>              <string>Dislexia</string>
  <key>CFBundleIdentifier</key>        <string>com.dislexia.app</string>
  <key>CFBundleExecutable</key>        <string>Dislexia</string>
  <key>CFBundlePackageType</key>       <string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key>           <string>1</string>
  <key>CFBundleIconFile</key>          <string>icon</string>
  <key>NSHighResolutionCapable</key>   <true/>
  <!-- No LSUIElement: Dock visibility is toggled at runtime (plan/08 §4). -->
</dict>
</plist>
PLIST
codesign --force --deep --sign - "$BUNDLE" # ad-hoc; local use
echo "Built ${BUNDLE} (v${VERSION})"
