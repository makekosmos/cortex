# Task: ARK object-first automatic migrations

## Context

The user confirmed that Delphi tasks and Eden notes should move to ARK object-first storage and that migration should happen automatically when the desktop app starts. The legacy Delphi Rust sidecar is replaced by the canonical `ark-core-rpc` runtime and ARK SQLite database.

## Acceptance Criteria

AC1. Delphi no longer packages or falls back to the legacy `delphi-db` sidecar; the only ARK runtime path is `ark-core-rpc`.

AC2. Delphi automatically migrates legacy todos into ARK `task_obj` objects on app entry and on shared-space switch.

AC3. Delphi task read/write/delete IPC paths use ARK objects as the primary task model after migration.

AC4. Eden automatically migrates Heart vault note types and entries into ARK object types and `note_obj`-compatible objects during store initialization.

AC5. Eden migrates note links only after all note objects are present, so relationship writes do not depend on migration order.

AC6. The removed legacy Delphi sidecar has a documented replacement: `ark-core-rpc` owns ARK SQLite `ark.db` and the generic object model.

AC7. Verification artifacts include Delphi, Eden, and package checks that can run locally, plus any blocked checks recorded with command output.
