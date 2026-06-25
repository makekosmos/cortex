# desloppify my-cosmos window dead export cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-dashboard-window-dead-export-cleanup/desloppify-after.json`
- score 9, total 248, HIGH 95, MEDIUM 109, LOW 44

After:

- `.agent/tasks/2026-06-24-desloppify-my-cosmos-window-dead-export-cleanup/desloppify-after.json`
- score 9, total 247, HIGH 94, MEDIUM 109, LOW 44

Rule delta:

- `DEAD_EXPORT`: 40 -> 39

Checks:

- `bun run --cwd platform/desktop typecheck`
- mojibake grep over `platform/desktop/electron/my-cosmos-window.ts`
