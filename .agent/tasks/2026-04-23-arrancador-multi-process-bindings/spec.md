# Arrancador Multi-Process Game Bindings Spec

## Task ID

2026-04-23-arrancador-multi-process-bindings

## Goal

Allow one Arrancador game to be associated with multiple tracked processes so Ark usage can be attributed to a single game across multiple executables or process-name-only tracker records.

## Scope

- Add a persistent process-binding model for games in the Arrancador local database.
- Aggregate playtime, play count, last played, and running-instance checks across the primary game executable plus additional bindings.
- Expose IPC/API methods to list, search, add, and remove process bindings.
- Add a game-detail UI flow to inspect current bindings and attach multiple bindings from recent/search results in the Ark usage tracker.
- Keep the active Vue renderer and Electron backend as the only supported runtime path.

## Acceptance Criteria

- AC1: A game can store multiple additional process bindings beyond its primary `exe_path`, and duplicate/ambiguous bindings are rejected defensibly.
- AC2: Ark usage hydration and playtime statistics attribute tracker rows to a game when they match either the primary executable path or any saved binding.
- AC3: Running-instance count and process termination respect all executable-path bindings for the game.
- AC4: Game detail UI shows current bindings, supports removing them, and supports opening a modal that:
  - shows up to 10 recent tracked processes initially,
  - performs debounced search against the usage tracker,
  - allows multi-select add in one action.
- AC5: Focused tests cover the new aggregation/binding behavior, and current verification commands pass for `apps/arrancador`.
- AC6: Evidence artifacts are written under `.agent/tasks/2026-04-23-arrancador-multi-process-bindings/`.

## Non-goals

- Do not redesign the library page.
- Do not replace the primary executable launch path with arbitrary binding selection.
- Do not import all tracker processes eagerly into the renderer.
