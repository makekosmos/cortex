# Evidence: Arrancador Ark Game Writes Via SDK

## Verdict

PASS

## Acceptance Criteria

- AC1: PASS. `syncGame()` writes via `arkObjects.upsert(...)`; `ark-game-objects.ts` no longer contains `INSERT OR REPLACE INTO objects`.
- AC2: PASS. Focused tests cover reuse by `ark_object_id`, `arrancador_game_id`, and executable path fallback.
- AC3: PASS. Arrancador now builds `ark-core-rpc` in sidecar scripts and packages it under `ark-core/ark-core-rpc.exe`.
- AC4: PASS. `apps/arrancador/package.json` declares `@arksync/node` as a workspace dependency.
- AC5: PASS. Read-only hydration still uses the existing readonly Ark DB path and was not removed.
- AC6: PASS. Fresh TypeScript, test, build, lint, and packaging checks are recorded in this task directory.

## Raw Artifacts

- `typecheck.txt`: `bun run typecheck` in `apps/arrancador`.
- `test-ark-game-objects.txt`: focused Vitest run for the new Ark game object service tests.
- `build-main.txt`: Arrancador Electron main bundle build.
- `build-sidecar-dev.txt`: dev build for `arrancador-sidecar` and `ark-core-rpc`.
- `build-sidecar-release.txt`: release build for `arrancador-sidecar` and `ark-core-rpc`.
- `arksync-node-typecheck.txt`: `@arksync/node` typecheck after exporting `JsonValue`.
- `arksync-node-build.txt`: `@arksync/node` build after exporting `JsonValue`.
- `biome-check.txt`: focused Biome check for touched Arrancador TS files.
- `git-diff-check.txt`: whitespace/error diff check for touched files.
- `no-direct-insert.txt`: proof that the touched service no longer writes `objects` by direct SQL.
- `source-evidence.txt`: source locations for SDK use, binary packaging, and dependency wiring.
- `git-diff.txt`: current patch for the task.
- `problems.md`: failed first-pass checks and fixes.

## Commands

```text
bun install
cmd /c "bun run typecheck > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\typecheck.txt 2>&1"
cmd /c "bun vitest run --configLoader native --config vitest.config.mjs electron/main/services/ark-game-objects.test.ts > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\test-ark-game-objects.txt 2>&1"
cmd /c "bun run build:main > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\build-main.txt 2>&1"
cmd /c "bun run build:sidecar:dev > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\build-sidecar-dev.txt 2>&1"
cmd /c "bun run build:sidecar > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\build-sidecar-release.txt 2>&1"
cmd /c "bun run typecheck > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\arksync-node-typecheck.txt 2>&1"
cmd /c "bun run build > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\arksync-node-build.txt 2>&1"
cmd /c "bun biome check electron/main/services/ark-game-objects.ts electron/main/services/ark-game-objects.test.ts > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\biome-check.txt 2>&1"
cmd /c "git diff --check -- apps/arrancador/electron/main/services/ark-game-objects.ts apps/arrancador/electron/main/services/ark-game-objects.test.ts apps/arrancador/package.json apps/arrancador/electron-builder.yml packages/arksync-node/src/index.ts .agent/tasks/2026-04-24-arrancador-ark-game-writes-sdk/spec.md > D:\Personal\Hobby\Coding\kosmos\.agent\tasks\2026-04-24-arrancador-ark-game-writes-sdk\git-diff-check.txt 2>&1"
```

## Notes

`git diff --check` returned exit code 0. It reported only Git line-ending warnings for touched files.
