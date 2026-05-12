# Evidence

Verification: PASS.

## Acceptance Criteria

- AC1 PASS: root `AGENTS.md` requires all tests, smoke checks, Playwright runs, and migration verification to use isolated test databases and never a main/user ARK database.
- AC2 PASS: Delphi task migration writes a one-per-version JSON backup snapshot when `backupDir` is provided.
- AC3 PASS: Delphi task migration returns and persists a structured report with status, counts, backup path, and per-record errors.
- AC4 PASS: Delphi task migration continues past individual `upsertObject` failures and does not write the success marker on partial failure.
- AC5 PASS: Eden note migration writes a one-per-version JSON backup snapshot when `backupDir` is provided.
- AC6 PASS: Eden note migration returns structured type/object/link counts, backup path, per-record errors, and continues past object/link failures.
- AC7 PASS: Delphi Vitest, Delphi Playwright, and Eden migration script cover backup/report/partial-failure behavior using temporary test data paths.
- AC8 PASS: fresh Delphi/Eden tests, builds, Playwright e2e, and diff checks were run and recorded.

## Fresh Checks

- PASS: `bun run --cwd apps/delphi/ts test`
- PASS: `bun run --cwd apps/delphi/ts build`
- PASS: `bun run --cwd apps/delphi/ts e2e`
- PASS: `bun run --cwd apps/eden/ts test:ark-migration`
- PASS: `bun run --cwd apps/eden/ts build`
- PASS: `git diff --check`
- PASS: `rg -n "Test database isolation|isolated test databases|main/user ARK database" AGENTS.md`

Raw command notes are in `raw/command-results.md`.
