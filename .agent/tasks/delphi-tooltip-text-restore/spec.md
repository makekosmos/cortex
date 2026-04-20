# Task: Delphi Tooltip Text Restore

## Goal
Restore broken Russian user-facing text in the Delphi desktop status tooltip/popover so the UI matches the last known good text from git history.

## Scope
- `apps/delphi/ts/src/App.vue`
- verification artifacts in `.agent/tasks/delphi-tooltip-text-restore/`

## Acceptance Criteria
- AC1: The `StatusDot` tooltip content in `apps/delphi/ts/src/App.vue` contains valid Russian text for the P2P connection popup and no mojibake in those restored labels.
- AC2: Restored tooltip/popover strings match the last known good wording from git history rather than ad-hoc rewrites.
- AC3: TypeScript verification for `apps/delphi/ts` passes after the change.

## Source of Truth
- Git history for `apps/delphi/ts/src/App.vue`, especially commit `0abe11d` (`better kepler ui`), which contains intact Russian strings for the same UI block.

## Notes
- Keep the fix minimal and localized to the broken text block unless current-code verification shows additional required edits.
