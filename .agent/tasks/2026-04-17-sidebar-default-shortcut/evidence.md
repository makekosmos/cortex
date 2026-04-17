# Evidence: Shared sidebar shortcut works by default everywhere

## Summary

- Restored a built-in default `toggleShortcut` in the shared [`Sidebar.vue`](/D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/components/Sidebar.vue).
- Eden keeps using `KeplerSidebar` without passing any `toggle-shortcut` prop, so it now inherits the same keyboard behavior as other apps.
- Focused type checks passed for dashboard, Delphi, and Eden consumers.

## Acceptance Criteria

### AC1

`packages/kepler-visuals/components/Sidebar.vue` uses a default toggle shortcut so keyboard toggling works even when consumers omit `toggleShortcut`.

Status: PASS

Proof:
- `Sidebar.vue` now defaults `toggleShortcut` to `meta+b|ctrl+b`.
- The shared matcher in the same component still handles layout aliases through `expandKeyVariants()` / `matchesShortcut()`.
- Raw artifact: [sidebar-default-shortcut.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-default-shortcut/raw/sidebar-default-shortcut.txt)

### AC2

Eden keeps using the shared sidebar without extra local shortcut props and receives the same behavior as Delphi/dashboard.

Status: PASS

Proof:
- [`apps/eden/ts/src/components/sidebar/EdenSidebar.vue`](/D:/Personal/Hobby/Coding/kepler/apps/eden/ts/src/components/sidebar/EdenSidebar.vue) still renders `KeplerSidebar` without a `toggle-shortcut` prop.
- The fix is centralized in shared `Sidebar.vue`, so Eden now inherits the same default shortcut automatically.
- Raw artifact: [eden-sidebar-consumer.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-default-shortcut/raw/eden-sidebar-consumer.txt)

### AC3

Focused type checks pass for current consumers after the change.

Status: PASS

Proof:
- `apps/dashboard` typecheck passed.
- `apps/delphi/ts` typecheck passed.
- `apps/eden/ts` typecheck passed.
- Raw artifacts:
  - [dashboard-tsc.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-default-shortcut/raw/dashboard-tsc.txt)
  - [delphi-tsc.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-default-shortcut/raw/delphi-tsc.txt)
  - [eden-tsc.txt](/D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-default-shortcut/raw/eden-tsc.txt)

## Checks Run

1. `bun run --cwd packages/kepler-visuals format`
2. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/dashboard/tsconfig.json`
3. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/delphi/ts/tsconfig.json`
4. `apps/dashboard/node_modules/.bin/tsc.exe --noEmit -p apps/eden/ts/tsconfig.json`

## Notes

- This change intentionally keeps consumer code unchanged and makes the keyboard shortcut a shared default again.
