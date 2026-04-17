# Evidence

## Verification Summary

- AC1 `PASS` — shared history-control component added at [packages/kepler-visuals/components/TitlebarHistoryControls.vue](D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/components/TitlebarHistoryControls.vue) and exported via [packages/kepler-visuals/components/index.ts](D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/components/index.ts) and [packages/kepler-visuals/index.ts](D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/index.ts).
- AC2 `PASS` — the component exposes explicit `backDisabled` / `forwardDisabled` props and styles disabled state through button opacity and `:disabled`.
- AC3 `PASS` — dashboard now derives history availability from Vue Router state and wires the shared controls in [apps/dashboard/src/components/dashboard/DashboardShell.vue](D:/Personal/Hobby/Coding/kepler/apps/dashboard/src/components/dashboard/DashboardShell.vue).
- AC4 `PASS` — Delphi TS now derives history availability from Vue Router state and wires the shared controls in [apps/delphi/ts/src/App.vue](D:/Personal/Hobby/Coding/kepler/apps/delphi/ts/src/App.vue).
- AC5 `PASS` — Eden now uses the same shared controls, but drives them from a local screen/entry history stack in [apps/eden/ts/src/App.vue](D:/Personal/Hobby/Coding/kepler/apps/eden/ts/src/App.vue).
- AC6 `PASS` — all typechecks passed:
  - `tsc --noEmit -p apps/dashboard/tsconfig.json`
  - `tsc --noEmit -p apps/delphi/ts/tsconfig.json`
  - `tsc --noEmit -p apps/eden/ts/tsconfig.json`
- AC7 `PASS` — docs updated in [apps/dashboard/AGENTS.md](D:/Personal/Hobby/Coding/kepler/apps/dashboard/AGENTS.md), [apps/delphi/AGENTS.md](D:/Personal/Hobby/Coding/kepler/apps/delphi/AGENTS.md) and [apps/eden/ts/AGENTS.md](D:/Personal/Hobby/Coding/kepler/apps/eden/ts/AGENTS.md).

## Notes

- `packages/kepler-visuals/components/Sidebar.vue` and `packages/kepler-visuals/components/sidebar.css` were already dirty before this feature pass due to ongoing sidebar work. They are visible in `git diff --stat`, but not modified for the titlebar history feature itself beyond existing workspace state.
- Eden does not use Vue Router for page history here, so its titlebar controls are bound to a local history stack of `activeScreen/currentEntryId/activeSpace` snapshots instead.
- Eden history recording now suppresses watcher writes while a stored snapshot is being applied, so `back` no longer re-enqueues the same state and `forward` is not cleared by its own programmatic transition.

## Raw Artifacts

- [raw/verification.txt](D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-titlebar-history-controls/raw/verification.txt)
