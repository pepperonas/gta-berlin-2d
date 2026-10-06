#!/usr/bin/env bash
# Baut „GTA Berlin.app“ (macOS) mit eigenem Dock-Icon: Release-Build, Bundle unter dist/, auf Wunsch nach
# ~/Applications kopiert. Die Karte lädt das Spiel aus dem Repo (Pfad steht beim Bauen fest) – das Bundle läuft
# also auf diesem Rechner, solange das Repo an seinem Platz liegt.
#   tools/macos-app.sh            Bundle in dist/
#   tools/macos-app.sh --install  zusätzlich nach ~/Applications
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -q -p gta-berlin
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
app="dist/GTA Berlin.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/gta-berlin "$app/Contents/MacOS/gta-berlin"
cp data/gfx/icon/GtaBerlin.icns "$app/Contents/Resources/GtaBerlin.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>GTA Berlin</string>
  <key>CFBundleDisplayName</key><string>GTA Berlin</string>
  <key>CFBundleIdentifier</key><string>io.celox.gta-berlin</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleExecutable</key><string>gta-berlin</string>
  <key>CFBundleIconFile</key><string>GtaBerlin</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.action-games</string>
</dict>
</plist>
PLIST
echo "Bundle: $app (Rust $version)"
if [[ "${1:-}" == "--install" ]]; then
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/GTA Berlin.app"
  cp -R "$app" "$HOME/Applications/"
  echo "Installiert: ~/Applications/GTA Berlin.app"
fi
