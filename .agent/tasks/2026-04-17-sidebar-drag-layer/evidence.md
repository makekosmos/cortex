# Evidence

## Verification Summary

- AC1 `PASS` — shared sidebar shell now enables window dragging through `.kepler-sidebar-shell--drag-region::before` in [packages/kepler-visuals/components/Sidebar.vue](D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/components/Sidebar.vue).
- AC2 `PASS` — drag remains a background pseudo-layer, while top/body/footer content stays above it and existing `no-drag` controls remain unchanged.
- AC3 `PASS` — resize-handle logic is untouched in [packages/kepler-visuals/components/sidebar.css](D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/components/sidebar.css), and the drag region lives under the shell content rather than on the handle.
- AC4 `PASS` — the fix is isolated to shared sidebar code in [packages/kepler-visuals/components/Sidebar.vue](D:/Personal/Hobby/Coding/kepler/packages/kepler-visuals/components/Sidebar.vue); no app-local CSS or layout hack was added.

## Checks

- `node ...typescript/lib/tsc.js --noEmit -p apps/dashboard/tsconfig.json` — `PASS`
- `git diff -- packages/kepler-visuals/components/Sidebar.vue` captured in raw verification

## Raw Artifacts

- [raw/verification.txt](D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-17-sidebar-drag-layer/raw/verification.txt)
