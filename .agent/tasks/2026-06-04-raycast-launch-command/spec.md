# Spec - Raycast launchCommand lifecycle slice

## Classification

FULL_LOOP. Raycast command lifecycle touches extension host dispatch, trusted
command execution, and shell command routing.

## Goal

Trusted Raycast-compatible commands can call `launchCommand()` from `@raycast/api`
and have Kosmos resolve the target through the existing declared-command
registry instead of throwing "launchCommand is not configured".

## Acceptance Criteria

- `launchCommand()` calls from trusted command code reach the command runner host
  adapter.
- A launched Raycast target receives `LaunchType.LaunchCommand`,
  `arguments`, `fallbackText`, and `context` as `LaunchProps`.
- Shell routing supports trusted Raycast `view` targets, trusted Raycast
  `no-view` targets, and existing declared `open` commands.
- User-installed Raycast JS remains blocked from main-process execution until an
  isolated runtime exists.
- Docs describe the support boundary.

## Out of Scope

- Permission prompt UI for cross-extension launches.
- Background/scheduled launch types.
- Rich navigation stack parity.
- Untrusted/user extension sandbox.
