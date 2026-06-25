# desloppify instance kind dead export cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-my-cosmos-window-dead-export-cleanup/desloppify-after.json`
- score 9, total 247, HIGH 94, MEDIUM 109, LOW 44

After:

- `.agent/tasks/2026-06-24-desloppify-instance-kind-dead-export-cleanup/desloppify-after.json`
- score 9, total 246, HIGH 93, MEDIUM 109, LOW 44

Rule delta:

- `DEAD_EXPORT`: 39 -> 38

Checks:

- `rg InstanceKind platform/desktop products packages tests core`
- `bun run --cwd platform/desktop typecheck`
- mojibake grep over `platform/desktop/electron/instance.ts`
