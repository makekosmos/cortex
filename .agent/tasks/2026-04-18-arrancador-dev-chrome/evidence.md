# Evidence - Arrancador dev startup and shared chrome pass

Verdict: PASS

## Root cause
- Confirmed from the pre-change script wiring that `apps/arrancador/package.json` previously executed `bun run rebuild:native` on every `bun run dev`.
- The expensive part was not `scripts/dev.ts`; it was the unconditional `electron-rebuild` of `better-sqlite3` before dev watchers started.
- A direct Vue import from `packages/kosmos-visuals` was not the minimal path because `arrancador` is a React renderer, so the shared shell had to be ported as a local React/Electron chrome implementation.

## Acceptance criteria
### AC1
PASS
- `predev` now points to the guarded script at `apps/arrancador/package.json:9`.
- The guard script computes current native state, compares it with the saved stamp, and skips rebuilds when unchanged: `apps/arrancador/scripts/rebuild-native-if-needed.ts:108-122`.
- Fresh verification: `bun run predev` returned `[native] native rebuild is up to date, skipping` in 119 ms.

### AC2
PASS
- The rebuild decision is deterministic and keyed off explicit compatibility inputs in `apps/arrancador/src/lib/native-rebuild.ts:25-67`.
- Compared inputs include stamp version, platform, arch, Electron version, better-sqlite3 version, artifact path, artifact size, artifact mtime, and artifact presence.
- Manual recovery path remains available through `rebuild:native` in `apps/arrancador/package.json:8`.
- Regression coverage for the decision function is in `apps/arrancador/src/test/rebuild-native-if-needed.test.ts`.

### AC3
PASS
- Desktop layout now composes a dedicated titlebar plus integrated sidebar shell in `apps/arrancador/src/pages/Layout.tsx:42-106`.
- The new titlebar is implemented in `apps/arrancador/src/components/AppTitlebar.tsx:51-173`.
- Sidebar behavior and persistence were adapted to a controllable kosmos-style shell in `apps/arrancador/src/components/Sidebar.tsx:17-222`.
- Mobile navigation remains intact through the preserved mobile top bar and slide-over sidebar in `apps/arrancador/src/pages/Layout.tsx:53-99`.

### AC4
PASS
- Renderer-facing window chrome helpers are exposed via `apps/arrancador/src/lib/api.ts` and `apps/arrancador/src/lib/window-chrome.ts`.
- IPC request/result contracts include platform detection and window controls in `apps/arrancador/src/types/ipc.ts`.
- Electron main registers the new handlers in `apps/arrancador/electron/main/backend.ts:682-701`.
- Window creation now enables custom titlebar behavior on macOS and Windows, with Windows overlay height syncing in `apps/arrancador/electron/main/windows.ts:75-136`.

### AC5
PASS
- Added regression coverage for the rebuild decision and titlebar shell behavior:
  - `apps/arrancador/src/test/rebuild-native-if-needed.test.ts`
  - `apps/arrancador/src/test/app-titlebar.test.tsx`
  - `apps/arrancador/src/test/layout.test.tsx`
  - `apps/arrancador/src/test/sidebar-component.test.tsx`
- Fresh verification on current code:
  - `bun run typecheck` -> PASS
  - `bun run test` -> PASS
  - `bun run build:main` -> PASS
  - `bun run build:preload` -> PASS
  - `bun run build:renderer` -> PASS
  - `bun run predev` -> PASS with native rebuild skip path

## Raw artifacts
- `raw/root-cause.txt`
- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/build-main.txt`
- `raw/build-preload.txt`
- `raw/build-renderer.txt`
- `raw/predev-skip.txt`

## Notes
- Direct interactive `bun run test` passes.
- Redirecting Bun/Vitest output through shell redirection in this Windows workspace can trigger a false Vite startup failure (`spawn EPERM` during config loading). This is a tooling artifact of the capture method, not a failure of the current `arrancador` code; the PASS verdict is based on the direct command result.
- Parallel sub-agent analysis was used to separate the startup bottleneck investigation from the `kosmos-visuals` chrome audit before implementation.
