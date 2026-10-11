#!/usr/bin/env bash
# Quarantine-free Mundus install for macOS:
#   curl -fsSL https://raw.githubusercontent.com/makekosmos/cortex/main/desktop/install-macos.sh | bash
#
# Files fetched with curl do not get com.apple.quarantine, so the copied app
# launches without Gatekeeper's download check. This is the supported install
# path until the DMG is notarized (KOS-349); on macOS 26 the browser-download
# route has no "Open Anyway" escape hatch for unnotarized apps.
set -euo pipefail

APP_NAME="Mundus Manager.app"
DEST="/Applications/$APP_NAME"

# sed, not python3: /usr/bin/python3 exists only with the Command Line Tools.
URL=$(curl -fsSL https://api.github.com/repos/makekosmos/cortex/releases/latest \
  | sed -n 's/.*"browser_download_url": *"\([^"]*\.dmg\)".*/\1/p' | head -1)
[ -n "$URL" ] || { echo "install-macos: could not find a DMG asset in the latest release" >&2; exit 1; }
echo "install-macos: $URL"

WORK=$(mktemp -d)
trap 'hdiutil detach "$WORK/mnt" -quiet 2>/dev/null; rm -rf "$WORK"' EXIT

curl -fL --progress-bar -o "$WORK/mundus.dmg" "$URL"
hdiutil attach -nobrowse -readonly -mountpoint "$WORK/mnt" "$WORK/mundus.dmg" >/dev/null

if pgrep -f "$DEST/Contents/MacOS/" >/dev/null; then
  echo "install-macos: Mundus is running — quit it first, then re-run this script." >&2
  exit 1
fi
rm -rf "$DEST"
ditto "$WORK/mnt/$APP_NAME" "$DEST"
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null || true
echo "install-macos: installed to $DEST — open it like any other app."
