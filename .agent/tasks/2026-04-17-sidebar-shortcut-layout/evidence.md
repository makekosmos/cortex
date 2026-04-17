# Evidence: Restore sidebar hide shortcut on `Ctrl+B` / `Ctrl+И`

## Summary

- Updated the shared shortcut matcher in [`packages/kepler-visuals/components/Sidebar.vue`](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/components/Sidebar.vue) to recognize keyboard-layout aliases between Latin and Russian keys.
- Kept consumer shortcut strings unchanged: apps still pass `meta+b|ctrl+b`.
- Revalidated the affected consumers with fresh TypeScript checks.

## Acceptance Criteria

### AC1

`Sidebar.vue` matches `Ctrl+B` / `Meta+B` reliably using both physical key codes and localized key values.

Status: PASS

Proof:
- `Sidebar.vue` now defines `keyLayoutAliases`, `reverseKeyLayoutAliases`, and `expandKeyVariants()`.
- `matchesShortcut()` now compares shortcut keys against normalized event key variants from both `event.key` and `event.code`.
- This covers the Latin key `b` and Russian layout value `и` as equivalent variants for the same shortcut.
- Raw artifact: [sidebar-shortcut-grep.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-shortcut-layout/raw/sidebar-shortcut-grep.txt)

### AC2

Existing consumers that pass `toggle-shortcut="meta+b|ctrl+b"` work without needing app-specific workaround strings.

Status: PASS

Proof:
- Delphi and dashboard consumers still pass the unchanged shortcut string `meta+b|ctrl+b`.
- The fix is inside shared `Sidebar.vue`, not in consumer-specific props.
- Raw artifact: [consumer-shortcuts.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-shortcut-layout/raw/consumer-shortcuts.txt)

### AC3

Current shared sidebar code and Delphi consumer typecheck successfully after the fix.

Status: PASS

Proof:
- `apps/dashboard` typecheck passed.
- `apps/delphi/ts` typecheck passed.
- Raw artifacts:
  - [dashboard-tsc.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-shortcut-layout/raw/dashboard-tsc.txt)
  - [delphi-tsc.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-shortcut-layout/raw/delphi-tsc.txt)

## Checks Run

1. `bun run --cwd packages/kepler-visuals format`
2. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/dashboard/tsconfig.json`
3. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/delphi/ts/tsconfig.json`

## Notes

- The current worktree already contains other uncommitted sidebar edits in `Sidebar.vue`; this task only extended the shared shortcut matching behavior and verified the current codebase state.
