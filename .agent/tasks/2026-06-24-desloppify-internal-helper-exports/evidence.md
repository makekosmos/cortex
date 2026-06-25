# Evidence

## Commands

- `rg countBooksInSection|projectGame|nextDate|formatPlaytimeSeconds|localImageUrl|EdenPreferences|GamePosterCardProps|AppCommandSetting incubator products packages platform tests`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension delphi`
- `bun run --cwd platform/desktop build:extension eden`
- `bun run --cwd platform/desktop build:extension arrancador`
- `bun run --cwd platform/desktop build:extension akasha`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-internal-helper-exports/desloppify-after.json`

## Results

- References check: PASS.
  - `projectGame`, `nextDate`, `formatPlaytimeSeconds`, `localImageUrl`,
    `EdenPreferences`, `GamePosterCardProps`, and the tab-local
    `AppCommandSetting` are only used inside their defining files.
  - `countBooksInSection` had no callers; `LibrarySection` remains exported and
    imported by Akasha components.
  - The externally imported `AppCommandSetting` from
    `platform/desktop/src/views/settings/navigation.ts` remains unchanged.
  - The Electron-side `localImageUrl` in `local-image-protocol.ts` remains
    unchanged.
- Eden typed-notes docs check: PASS. The changes do not alter type metadata,
  ARK calls, or image/object data paths.
- `typecheck`: PASS.
- `build:extension delphi`: PASS.
- `build:extension eden`: PASS.
- `build:extension arrancador`: PASS.
- `build:extension akasha`: PASS.
- Baseline scan: score 0, 496 findings; severity critical 0, high 295,
  medium 130, low 71.
- Final scan: score 0, 488 findings; severity critical 0, high 287,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 8 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Scoped internal-only exports are removed.
- AC3: PASS. Runtime helpers remain local; externally imported types/APIs remain
  exported.
- AC4: PASS. Relevant checks passed.
