#!/usr/bin/env bash
# Builds the Linux AppImage and .deb into release/ (run on Linux, see the system packages in
# .github/workflows/release.yml). budsctl is bundled next to the app.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(node -p "require('./src-tauri/tauri.conf.json').version")

[ "${SKIP_INSTALL:-}" = "1" ] || npm ci
npm run tauri build -- --bundles deb,appimage --config src-tauri/tauri.bundle.conf.json

mkdir -p release
shopt -s nullglob
for f in target/release/bundle/appimage/*.AppImage; do
  cp "$f" "release/BudsControl-${version}-linux-x64.AppImage"
done
for f in target/release/bundle/deb/*.deb; do
  cp "$f" "release/BudsControl-${version}-linux-x64.deb"
done
ls -l release
