# desloppify Delphi qrcode dependency cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-normalize-passphrase-dead-export-cleanup/desloppify-after.json`
- score 9, total 219, HIGH 70, MEDIUM 106, LOW 43

After:

- `.agent/tasks/2026-06-24-desloppify-delphi-qrcode-dependency-cleanup/desloppify-after.json`
- score 9, total 218, HIGH 70, MEDIUM 105, LOW 43

Rule delta:

- `DEAD_DEPENDENCY`: 16 -> 15

Changes:

- Removed unused direct `qrcode` dependency from `products/delphi/package.json`.
- Ran root `bun install`; root `bun.lock` updated.

Checks:

- `rg qrcode products/delphi package/lock files` found no Delphi runtime imports.
- `bun run --cwd platform/desktop typecheck` passed.
- `desloppify scan --json` produced the after JSON; exit 1 is expected while findings remain.
