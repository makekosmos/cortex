# desloppify Eden + Delphi dead files cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-instance-kind-dead-export-cleanup/desloppify-after.json`
- score 9, total 246, HIGH 93, MEDIUM 109, LOW 44

After:

- `.agent/tasks/2026-06-24-desloppify-eden-delphi-dead-files-cleanup/desloppify-after.json`
- score 9, total 222, HIGH 73, MEDIUM 106, LOW 43

Rule deltas:

- `DEAD_FILE`: 39 -> 19
- `BARREL_FILE`: 2 -> 1
- `LONG_FILE`: 22 -> 21
- `FAKE_LOADING_DELAY`: 6 -> 5
- `EMPTY_ARRAY_FALLBACK`: 33 -> 32

Checks:

- `rg` over removed file/symbol names: no code refs remain; only stale docs refs were updated.
- `bun run --cwd platform/desktop typecheck` passed.
- `bun run --cwd products/delphi test:vue` failed on existing UI class assertions (`rounded-md`, `rounded-xl`), not missing imports.
- `desloppify scan --json` produced the after JSON; exit 1 is expected while findings remain.

Held back:

- Delphi sync/protocol files.
- Eden editor-adjacent dormant files (`BlockSelectionOverlay.vue`, `useBlockSelection.ts`, `TaskStatusIcon.vue`, `WikilinkList.vue`).
- Site/scripts/incubator manual utilities and implicit entrypoints.
