# Spec - Raycast Action.LaunchCommand slice

## Classification

FULL_LOOP. The change connects Raycast UI actions to shell command lifecycle and
main-process routing.

## Goal

Trusted Raycast-compatible view commands can expose `Action.LaunchCommand` in an
`ActionPanel`, and the Kosmos Raycast host can execute it through the same
declared-command registry path as imperative `launchCommand()`.

## Acceptance Criteria

- `@raycast/api` exposes `Action.LaunchCommand`.
- `ActionPanel` model includes `Action.LaunchCommand`.
- List, Grid, and Form hosts can submit `Action.LaunchCommand` through guarded
  Raycast session IPC.
- Main-process Raycast view host resolves the action with its session
  `launchCommand` adapter.
- Docs mention the supported action.

## Out of Scope

- Permission prompt UI for cross-extension command launches.
- Keyboard shortcut parity.
- Rich navigation stack reuse inside the same host window.
