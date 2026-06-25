# Knip Electron entry false-positive cleanup

Add a minimal Knip workspace override for `platform/desktop` so the Electron
source entry is recognized without changing runtime code.

Config:

- `workspaces["platform/desktop"].entry = ["electron/main.ts"]`
- `workspaces["platform/desktop"].project = ["electron/**/*.ts"]`

Goal: remove false-positive `DEAD_FILE` findings for the main Electron source
files under `platform/desktop/electron/`.
