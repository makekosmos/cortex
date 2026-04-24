# Evidence

Task ID: `2026-04-23-arrancador-save-locator-atomization`

## Verification Result

Overall: `PASS`

## Acceptance Criteria

### AC1: `save-locator.ts` no longer owns root-resolution details

PASS.

- `apps/arrancador/electron/main/services/backup/save-locator.ts` is now focused on:
  - deduplicating save roots,
  - collecting files from roots,
  - exposing public save locator APIs,
  - save path template/token helpers.
- File size dropped from 633 lines to 184 lines.

### AC2: Root-resolution module owns path context and heuristics

PASS.

- Added `apps/arrancador/electron/main/services/backup/save-root-resolver.ts`.
- It owns:
  - path resolution context,
  - environment token expansion,
  - glob resolution,
  - Steam library/userdata discovery,
  - manifest root resolution,
  - heuristic root resolution.

### AC3: Public save-locator functions remain stable

PASS.

- Existing exports remain available:
  - `findGameSaveRoots`
  - `findGameSaves`
  - `findSavePath`
  - `discoverBackupInfo`
  - `resolveSavePathTemplate`
  - `tokenizeSavePath`

### AC4: Electron main tests cover save locator behavior

PASS.

- Added `apps/arrancador/electron/main/services/backup/save-locator.test.ts`.
- Tests cover:
  - explicit override root file discovery and total size,
  - save-path lookup candidates,
  - resolving and tokenizing `{PATHTOGAME}` paths.

### AC5: Fresh verification passes

PASS.

- `bun run typecheck`: PASS, raw `raw/typecheck.txt`.
- `bun run test`: PASS, 23 files / 69 tests, raw `raw/test.txt`.
- `bun run biome:check`: PASS, raw `raw/biome-check.txt`.
- `bun run build:main`: PASS, raw `raw/build-main.txt`.
- `bun run build:preload`: PASS, raw `raw/build-preload.txt`.

### AC6: Evidence artifacts exist

PASS.

Raw artifacts:

- `raw/typecheck.txt`
- `raw/test.txt`
- `raw/biome-check.txt`
- `raw/build-main.txt`
- `raw/build-preload.txt`
- `raw/diff.txt`
- `raw/status.txt`
- `raw/metrics.txt`

## Metrics

```text
184 apps/arrancador/electron/main/services/backup/save-locator.ts
466 apps/arrancador/electron/main/services/backup/save-root-resolver.ts
 68 apps/arrancador/electron/main/services/backup/save-locator.test.ts
```
