# desloppify normalize passphrase dead export cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-small-export-deexport-cleanup/desloppify-after.json`
- score 9, total 220, HIGH 71, MEDIUM 106, LOW 43

After:

- `.agent/tasks/2026-06-24-desloppify-normalize-passphrase-dead-export-cleanup/desloppify-after.json`
- score 9, total 219, HIGH 70, MEDIUM 106, LOW 43

Rule delta:

- `DEAD_EXPORT`: 36 -> 35

Checks:

- `bun run --cwd platform/desktop typecheck` passed.
- targeted grep confirmed `normalizePassphrase` was removed.
- mojibake grep over `normalize.ts` passed.
- `desloppify scan --json` produced the after JSON; exit 1 is expected while findings remain.
