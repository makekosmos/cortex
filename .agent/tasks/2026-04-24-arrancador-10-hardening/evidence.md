# Evidence

Task: `2026-04-24-arrancador-10-hardening`

## Result

Verification status: `PASS_PENDING_FINAL_REVIEWS`

All product-code and command-verification acceptance criteria pass in the current
codebase. AC9 remains pending until five fresh independent post-fix reviews
return 10/10 in architecture, readability, atomicity, and testing.

## Acceptance Criteria

AC1 PASS. `changed-files.md` separates proof artifacts, task-owned hardening
files, and related broader/pre-existing dirty state.

AC2 PASS. `bun run typecheck`:
- raw: `raw/typecheck.txt`
- command: `vue-tsc --noEmit && tsc --noEmit -p tsconfig.electron.json && tsc --noEmit -p tsconfig.node.json`

AC3 PASS. `electron/main/services/games.ts` no longer has unresolved
`getRowById` or `queryOne` hazards.
- `queryOne` is imported explicitly.
- `recordGameLaunch` uses `createGameLoaders(...).loadGame`.
- focused coverage exists in `electron/main/services/games.test.ts`.

AC4 PASS. Backup copy/restore path safety is enforced.
- TypeScript copy rejects traversal, absolute paths, drive-qualified paths,
  unsafe segments, and unsafe root labels.
- Rust copy rejects unsafe relative paths and root labels.
- TypeScript restore rejects unsafe manifest `backupPath` values and rejects
  `originalPath` outside trusted `allowedRestoreRoots`.
- Rust restore rejects unsafe manifest `backup_path` values and rejects
  `original_path` outside trusted restore roots.
- Rust restore normalizes allowed restore roots across `/` and `\` separator
  forms before the trusted-root comparison.
- Rust restore rejects empty manifest `backup_path` segments, matching the
  TypeScript restore path policy.
- tests: `backup/copy.test.ts`, `backup/restore.test.ts`, `sidecar/src/backup.rs`.

AC5 PASS. Managed sidecar request handling is race-safe.
- stdin stream/write errors are captured.
- stdout draining is serialized with `stdoutDrain`.
- close handling waits for stdout draining before rejecting.
- the sidecar process is spawned directly with `shell: false`.
- protocol tests cover stdin stream errors and write callback errors.
- protocol tests cover delayed scan events, copy/restore progress, malformed
  responses, failed responses, and abort.

AC6 PASS. Vue template contracts are checked by normal typecheck.
- `package.json` typecheck starts with `vue-tsc --noEmit`.
- `GameDetailPage` passes `saving-edit`, `metadata-results`,
  `searching-metadata`, and `applying-metadata` to `GameDetailDialogs`.
- `architecture-boundaries.test.ts` covers the prop bridge.

AC7 PASS. Coverage includes active Electron main/service facades and nested
implementation files.
- raw coverage summary: `raw/coverage.txt`
- copied lcov: `raw/coverage-lcov.info`
- lcov includes:
  - `electron\main\ipc\backup-handlers.ts`
  - `electron\main\services\backup\index.ts`
  - `electron\main\services\backup\copy.ts`
  - `electron\main\services\backup\restore.ts`
  - `electron\main\services\backup-workflow\service.ts`
  - `electron\main\services\backup-workflow\write-use-cases.ts`
  - `electron\main\services\backup-workflow\read-use-cases.ts`
  - `electron\main\services\games.ts`
  - `electron\main\sidecar\arrancador-sidecar.ts`

AC8 PASS. Verification commands and raw logs:
- `bun run typecheck`: PASS, `raw/typecheck.txt`
- `bun run lint`: PASS, `raw/lint.txt`
- `bun run test`: PASS, 45 files / 152 tests, `raw/test.txt`
- `bun run test:coverage`: PASS, `raw/coverage.txt`
  - statements 80.99%
  - branches 71.38%
  - functions 77.46%
  - lines 82.00%
- `bun run build:renderer`: PASS, `raw/build-renderer.txt`
- `bun run build:main`: PASS, `raw/build-main.txt`
- `bun run build:preload`: PASS, `raw/build-preload.txt`
- `cargo test --manifest-path sidecar/Cargo.toml`: PASS, 11 tests,
  `raw/cargo-test.txt`
- sandbox `bun run test:e2e`: attempted, blocked by EPERM spawning Playwright
  worker, `raw/e2e.txt`
- escalated `bun run test:e2e`: PASS, 4 Playwright tests, `raw/e2e-escalated.txt`

AC9 PENDING. Five independent post-fix reviews must run after this evidence
update. Completion requires all five to return 10/10 in every category.
