#!/usr/bin/env bash
# Quarantine-free Mundus install for macOS:
#   curl -fsSL https://raw.githubusercontent.com/makekosmos/cortex/main/desktop/install-macos.sh | bash
#
# Files fetched with curl do not get com.apple.quarantine, so the copied app
# launches without Gatekeeper's download check. This is the supported install
# path until the DMG is notarized (KOS-349); on macOS 26 the browser-download
# route has no "Open Anyway" escape hatch for unnotarized apps.
#
# The swap mirrors the self-updater (runtime/src/updater/macos.rs): stop a
# running Mundus through the Engine's own --shutdown, replace the bundle via a
# same-volume staging dir with rollback, relaunch.
set -euo pipefail

APP_NAME="Mundus Manager.app"
DEST="${MUNDUS_INSTALL_DIR:-/Applications}/$APP_NAME"
FEED="https://github.com/makekosmos/cortex/releases/latest/download"

say() { echo "install-macos: $*"; }
die() { echo "install-macos: $*" >&2; exit 1; }

WORK=$(mktemp -d)
STAGING="$(dirname "$DEST")/.mundus-install-$$"
trap 'hdiutil detach "$WORK/mnt" -quiet 2>/dev/null; rm -rf "$WORK" "$STAGING"' EXIT

# plutil, not python3: /usr/bin/python3 exists only with the Command Line Tools.
curl -fsSL -o "$WORK/manifest.json" "$FEED/manifest.json"
field() { plutil -extract "$1" raw -o - "$WORK/manifest.json"; }
VERSION=$(field version)
FILE=$(field platforms.mac.file) || die "the latest release has no macOS build"
SHA512=$(field platforms.mac.sha512)

say "downloading Mundus $VERSION"
curl -fL --progress-bar -o "$WORK/$FILE" "$FEED/$FILE"
[ "$(openssl dgst -sha512 -binary "$WORK/$FILE" | base64)" = "$SHA512" ] \
  || die "$FILE does not match the sha512 in manifest.json"

hdiutil attach -nobrowse -readonly -mountpoint "$WORK/mnt" "$WORK/$FILE" >/dev/null 2>&1 \
  || die "could not mount $FILE"
mkdir "$STAGING"
ditto "$WORK/mnt/$APP_NAME" "$STAGING/$APP_NAME"

running() { pgrep -f "$DEST/Contents/MacOS/" >/dev/null 2>&1; }
if running; then
  say "stopping the running Mundus"
  "$DEST/Contents/MacOS/mundus-engine" --shutdown >/dev/null 2>&1 || true
  for _ in $(seq 150); do running || break; sleep 0.2; done
  pkill -f "$DEST/Contents/MacOS/" 2>/dev/null || true
  sleep 1
  pkill -9 -f "$DEST/Contents/MacOS/" 2>/dev/null || true
fi

if [ -e "$DEST" ]; then mv "$DEST" "$STAGING/old.app"; fi
if ! mv "$STAGING/$APP_NAME" "$DEST"; then
  [ -e "$STAGING/old.app" ] && mv "$STAGING/old.app" "$DEST"
  die "could not replace $DEST"
fi

say "installed Mundus $VERSION to $DEST"
[ -n "${MUNDUS_INSTALL_NO_OPEN:-}" ] || open "$DEST"
