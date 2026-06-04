# Spec - Raycast file/system actions slice

## Classification

FULL_LOOP. This extends the Raycast compatibility runtime across the API shim,
Electron runner, guarded session IPC, and renderer action surfaces.

## Goal

Trusted Raycast-compatible commands can expose and execute file/system actions
through Kosmos shell adapters without bypassing the existing trusted-source
boundary.

## Acceptance Criteria

- `@raycast/api` exports runtime-backed `open`, `showInFinder`, and `trash`.
- `Action.Paste`, `Action.ShowInFinder`, and `Action.Trash` serialize into
  host-renderable action nodes.
- Trusted no-view commands bridge system utilities to the shell adapter.
- Trusted view commands receive the same system adapter for registered action
  callbacks.
- List, Grid, and Form hosts display and dispatch the new action types.
- Electron session IPC handles paste, show-in-folder, and trash requests with
  guarded session validation.
- Unit tests cover API exports, command-runner bridge behavior, and view-model
  action collection.
- Docs mention the supported boundary.

## Out of Scope

- Untrusted Raycast JS sandbox.
- Finder-specific parity beyond Windows shell equivalents.
- Multi-application `Action.Open` targeting.
- Confirmation dialogs before trashing files.
