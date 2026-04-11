# Task Spec: Delphi TS P2P Sync Cleanup

## Context

Delphi TS currently mixes the new unified Rust P2P runtime (`lan-sync:*` via Electron main / `@arksync/node`) with legacy renderer-era sync code:

- `arkSync` relay bootstrap and Ark pairing UI remain in `App.vue` and `SettingsPage.vue`
- `broadcastToPeers` / `peer:*` renderer bridge code remains alongside `lan-sync:*`
- stores still emit both legacy and current sync broadcasts on local mutations

This creates overlapping sync paths and inconsistent runtime assumptions, which matches the observed behavior where peer connection and CRDT propagation work intermittently.

## Goal

Make Delphi TS use a single sync model for Electron P2P spaces:

- one runtime path: `lan-sync:*` / Rust backend
- one renderer event path: `lan-sync:*`
- no visible legacy Ark relay configuration in Delphi TS settings
- while preserving the existing non-Electron/web bootstrap path

## Non-Goals

- No changes to Android/iOS sync flows
- No protocol changes to the Rust mesh / CRDT implementation unless verification reveals a small required fix
- No unrelated cleanup in user-modified files outside sync-related Delphi TS surfaces
- Do not remove browser-only Ark HTTP bootstrap if `build:web` still depends on it

## Acceptance Criteria

- AC1: Delphi TS renderer no longer boots, reconnects, or sends local mutations through legacy `arkSync` / `broadcastToPeers` paths in Electron P2P mode; local changes use only `lan-sync:broadcastChange`.
- AC2: Delphi TS settings no longer expose legacy Ark Server pairing / connect / disconnect UI or persistence helpers for desktop P2P usage.
- AC3: Renderer lifecycle for space activation / leave / peer status uses only the current `lan-sync:*` flow; leftover `peer:*` bridge/event dependencies are removed from the active Electron path.
- AC4: Verification shows no remaining legacy Electron sync UI/runtime references (`broadcastToPeers`, `peer:*`, Ark relay settings UI) in Delphi TS, while non-Electron/web bootstrap remains functional; targeted tests/typecheck for touched files pass or are explicitly documented if blocked.

## Verification Plan

- Search for residual Delphi TS references to removed Electron legacy paths (`broadcastToPeers`, `peer:*`, removed Ark settings UI) and separately verify browser bootstrap helpers remain present where intended
- Run targeted tests/typecheck for touched Delphi TS files
- Review Electron main / renderer runtime boundaries after cleanup
