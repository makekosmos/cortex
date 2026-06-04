# Raycast Navigation Actions

## Goal

Add Raycast-compatible `Action.Pop` and `Action.PopToRoot` wrappers that execute through the existing trusted navigation runtime and host action panel.

## Acceptance Criteria

- `packages/raycast-api` exposes `Action.Pop` and `Action.PopToRoot`.
- Both actions register runtime-backed callbacks via `navigationPop()` / `navigationPopToRoot()`.
- Host model treats both actions as executable action nodes and preserves shortcuts.
- Host action panels show Russian fallback titles for both actions.
- List, Grid, Detail, and Form hosts forward both actions through guarded session IPC.
- Unit and visual verification cover API callbacks, model collection, command-runner callback dispatch, and UI execution.
