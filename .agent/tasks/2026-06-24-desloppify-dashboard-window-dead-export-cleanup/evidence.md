# desloppify dashboard window dead export cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-notifications-local-image-dead-exports-cleanup/desloppify-after.json`
- score 9, total 249, HIGH 96, MEDIUM 109, LOW 44

After:

- `.agent/tasks/2026-06-24-desloppify-dashboard-window-dead-export-cleanup/desloppify-after.json`
- score 9, total 248, HIGH 95, MEDIUM 109, LOW 44

Rule delta:

- `DEAD_EXPORT`: 41 -> 40

Checks:

- `bun test platform/desktop/electron/local-image-protocol.test.ts platform/desktop/electron/settings-autostart.test.ts`
- `bun run --cwd platform/desktop typecheck`
- mojibake/no-op grep over touched Electron files
