# Evidence

Task ID: `2026-04-23-arrancador-backend-readability-pass`

## Verification Result

Overall: `PASS`

## Acceptance Criteria

### AC1: `backend.ts` no longer owns the service graph

PASS.

- Added `apps/arrancador/electron/main/runtime-services.ts`.
- Moved detailed creation of games/settings/stats/metadata/catalogue/system/notification/Achievement/Ark usage services into `createRuntimeServices()`.
- `backend.ts` now stays focused on Electron app paths, runtime state, startup backfill, Ark binding refresh, IPC registration, and startup sync.

### AC2: Runtime behavior and public IPC names remain unchanged

PASS.

- No IPC command names were added, removed, or renamed.
- Existing handler registration modules are still called from `backend.ts`.
- `bun run typecheck`, `bun run test`, and all build checks pass.

### AC3: Dense IPC handlers are readable blocks

PASS.

- Reformatted:
  - `apps/arrancador/electron/main/ipc/app-handlers.ts`
  - `apps/arrancador/electron/main/ipc/game-handlers.ts`
  - `apps/arrancador/electron/main/ipc/shell-scan-handlers.ts`
- Added small local payload aliases/helpers where they reduce long inline types.

### AC4: Architecture tests guard the boundary

PASS.

- Extended `apps/arrancador/src-vue/test/architecture-boundaries.test.ts`.
- New checks assert:
  - `backend.ts` imports `runtime-services`.
  - `runtime-services.ts` exports `createRuntimeServices`.
  - backend does not import detailed service factories.
  - high-traffic IPC modules stay under the readability line-length guard.

### AC5: Fresh verification passes

PASS.

- `bun run typecheck`: PASS, raw `raw/typecheck.txt`.
- `bun run test`: PASS, 22 files / 66 tests, raw `raw/test.txt`.
- `bun run biome:check`: PASS, raw `raw/biome-check.txt`.
- `bun run build:renderer`: PASS, raw `raw/build-renderer.txt`.
- `bun run build:main`: PASS, raw `raw/build-main.txt`.
- `bun run build:preload`: PASS, raw `raw/build-preload.txt`.

### AC6: Evidence artifacts exist

PASS.

Raw artifacts:

- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/biome-check.txt`
- `raw/build-renderer.txt`
- `raw/build-main.txt`
- `raw/build-preload.txt`
- `raw/diff.txt`
- `raw/status.txt`
- `raw/metrics.txt`

## Metrics

Current focused backend file sizes:

```text
161 apps/arrancador/electron/main/backend.ts
138 apps/arrancador/electron/main/runtime-services.ts
179 apps/arrancador/electron/main/ipc/app-handlers.ts
239 apps/arrancador/electron/main/ipc/game-handlers.ts
131 apps/arrancador/electron/main/ipc/shell-scan-handlers.ts
```
