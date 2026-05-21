# 2026-04-24 Arrancador 10/10 Hardening

## Goal

Bring `apps/arrancador` to a defensible 10/10 quality target for architecture,
readability, atomicity, and testing within the current repository state.

## Scope

- Normal verification must cover Vue templates, Electron main/preload, Node scripts,
  unit tests, coverage, builds, Rust sidecar, and e2e.
- Known runtime/security hazards in Arrancador must be removed.
- Proof artifacts must describe the current code and current command results.
- Existing unrelated dirty worktree changes must not be reverted.

## Acceptance Criteria

AC1. `changed-files.md` separates task-owned hardening files from unrelated or
pre-existing dirty worktree state.

AC2. `bun run typecheck` from `apps/arrancador` typechecks Vue SFC templates,
renderer/shared TypeScript, Electron main/preload TypeScript, and Node scripts.

AC3. Runtime hazards in `electron/main/services/games.ts` are removed:

- no unresolved `getRowById`
- no unresolved `queryOne`
- `recordGameLaunch` has focused coverage or an explicit no-op contract.

AC4. Backup copy/restore path safety is enforced in TypeScript and Rust:

- backup-relative paths reject traversal, absolute paths, drive-qualified paths,
  unsafe segments, and unsafe root labels
- restore manifest `backupPath` rejects traversal, absolute paths, and
  drive-qualified paths
- restore manifest `originalPath` must stay within trusted allowed restore roots
  derived from the current game/save-root context.

AC5. Managed sidecar request handling is race-safe:

- stdin errors are handled
- stdout event/response draining is serialized
- process close waits for stdout draining before rejecting.

AC6. Vue template prop contracts are checked by the normal typecheck path, and
the `GameDetailPage` -> `GameDetailDialogs` prop bridge is covered.

AC7. Coverage includes the active Electron main/service facades and nested
implementation files relevant to the hardening work:

- backup IPC handlers
- backup service facade
- backup copy/restore
- backup workflow facade and nested modules
- games service facade
- sidecar protocol wrapper.

AC8. Verification passes and raw logs are captured under
`.agent/tasks/2026-04-24-arrancador-10-hardening/raw/`:

- `bun run typecheck`
- `bun run lint`
- `bun run test`
- `bun run test:coverage`
- `bun run build:renderer`
- `bun run build:main`
- `bun run build:preload`
- `cargo test --manifest-path sidecar/Cargo.toml`
- `bun run test:e2e` sandbox attempt and escalated rerun if sandbox blocks
  process spawning.

AC9. Five independent post-fix reviews with minimal context score architecture,
readability, atomicity, and testing. Completion requires all five to return
10/10 in every category with no unresolved material blocker.

## Non-goals

- Rewrite the app from scratch.
- Replace Electron, Vue, Bun, Vitest, Playwright, or Rust sidecar tooling.
- Delete user work or unrelated generated artifacts.
