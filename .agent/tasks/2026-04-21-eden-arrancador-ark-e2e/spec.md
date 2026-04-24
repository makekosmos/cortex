# Eden Arrancador Ark Integration E2E

## Summary

Add an end-to-end verification flow for the shared Ark DB scenario between `Arrancador` and `Eden`:

1. create a fully isolated temporary space;
2. create a test game;
3. sync that game into the shared `ark.db`;
4. verify that `Eden`, launched under the same `selected-space`, can see the game as `game_obj`;
5. remove the temporary DBs and vault after the check.

## Acceptance Criteria

- AC1: The repository contains an automated Playwright Electron test for the shared Ark DB scenario between `Arrancador` and `Eden`.
- AC2: The test creates a fully isolated temporary environment: temp vault, temp appdata, temp Arrancador DB.
- AC3: The test creates a game, syncs it into Ark, and verifies the presence of a `game_obj` row in `ark.db`.
- AC4: The test verifies that the synced game appears in `Eden`.
- AC5: The test performs cleanup of temporary files, and the verification run completes successfully.
