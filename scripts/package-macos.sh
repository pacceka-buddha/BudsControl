#!/usr/bin/env bash
# Builds the macOS .app and .dmg for the architecture of this Mac into release/ (run on macOS). The app is
# signed ad hoc ("-"), not notarised: Gatekeeper asks the user to confirm the first launch.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(node -p "require('./src-tauri/tauri.conf.json').version")
case "$(uname -m)" in
  arm64) arch=arm64 ;;
  x86_64) arch=x64 ;;
  *) arch=$(uname -m) ;;
esac

[ "${SKIP_INSTALL:-}" = "1" ] || npm ci
npm run tauri build -- --bundles app,dmg --config src-tauri/tauri.bundle.conf.json

mkdir -p release
shopt -s nullglob
for f in target/release/bundle/dmg/*.dmg; do
  cp "$f" "release/BudsControl-${version}-macos-${arch}.dmg"
done
ls -l release
