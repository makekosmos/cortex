# Evidence

Verification: PASS

## Acceptance Criteria

- AC1 PASS: root `package.json` exposes `ark:smoke`.
- AC2 PASS: `scripts/ark-smoke.mjs` creates the Dashboard smoke DB under `.agent/tasks/2026-04-26-ark-initial-plan-close/smoke`; Eden e2e uses temp app-data paths from the existing launcher.
- AC3 PASS: `ark:smoke` covers ARK core tests, usage-tracker tests, `@kosmos/ark` typecheck, Arrancador tests/typecheck, Eden migration/build/typed-note e2e, and Dashboard seed/analytics smoke.
- AC4 PASS: `docs/EDEN-HEART-ARK-BOUNDARY.md` and Eden typed-note docs explain that Heart remains the Rust layer for heavy editor/vault/search work while ARK owns shared object data.
- AC5 PASS: `docs/ARK-READONLY-SQL-BOUNDARY.md`, `apps/README.md`, and app docs explain read-only SQL boundaries and forbidden app writes.
- AC6 PASS: `scripts/check-ark-write-boundaries.mjs` detects direct app-service writes to ARK tables; `bun run ark:guard:writes` passed.
- AC7 PASS: fresh verification ran and is recorded here, in `evidence.json`, and in `raw/command-results.md`.

## Checks

- PASS: `bun run ark:guard:writes`
- PASS: `bun run ark:smoke`
- PASS: `git diff --check` with CRLF warnings only

## Smoke Result

`bun run ark:smoke` completed successfully. The Dashboard smoke DB path was:

`D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-26-ark-initial-plan-close\smoke\dashboard\smoke-dashboard.db`
