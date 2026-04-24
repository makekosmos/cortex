# Eden Game Note Presentation Cleanup

## Summary

Refine `game_obj` presentation in Eden so game notes stop exposing noisy Arrancador/system fields in the header by default and show only the useful human-facing fields.

## Acceptance Criteria

- AC1: New built-in `game_obj` defaults prefer a minimal header presentation focused on `play_status`, `genres`, `total_playtime_seconds`, and `last_played_at`.
- AC2: Existing spaces automatically get the cleaner `game_obj` presentation in Eden when their stored UI schema still matches the old noisy built-in preset.
- AC3: `TypedHeader` no longer renders hidden `background_image` data, and no longer shows the fallback type icon tile when no cover is shown.
- AC4: Read-only `game_obj` values render in a human-friendly format:
  - `play_status` as readable labels
  - `total_playtime_seconds` as readable playtime
  - `last_played_at` as a readable Russian date
- AC5: Type collection/list views use the same human-friendly formatting for those values.
- AC6: Verification covers the touched files and checks for mojibake regressions in new task artifacts.
