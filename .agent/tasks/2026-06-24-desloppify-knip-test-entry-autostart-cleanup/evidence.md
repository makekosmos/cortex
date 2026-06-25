# desloppify knip test entry + autostart cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-knip-desktop-entry-hooks-cleanup/desloppify-after.json`
- score 9, total 292, HIGH 138, MEDIUM 110, LOW 44

After:

- `.agent/tasks/2026-06-24-desloppify-knip-test-entry-autostart-cleanup/desloppify-after.json`
- score 9, total 251, HIGH 98, MEDIUM 109, LOW 44

Rule deltas:

- `DEAD_FILE`: 68 -> 39
- `DEAD_EXPORT`: 54 -> 43
- `DEAD_DEPENDENCY`: 17 -> 16

Checks:

- `bun test platform/desktop/electron/settings-autostart.test.ts`
- `bun run --cwd platform/desktop typecheck`
- mojibake grep over `knip.json`, `settings-autostart.ts`, `settings-autostart.test.ts`
