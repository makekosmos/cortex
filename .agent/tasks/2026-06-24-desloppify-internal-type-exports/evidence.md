# Evidence

## Commands

- `rg SyncNodeDeviceKind|SyncNodeStatus|DictationProvider|ConnectivityStage|DictationLocalModelInfo|VimMotionItem|LaunchResultOk|LaunchResultErr|RawgSearchResult|SqobaListResult|SqobaRestoreResult|OkResult|LaunchResult|VIM_MOTION_GROUPS|createDictationConfig|requireArrancadorApi packages platform products incubator tests`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension eden`
- `bun run --cwd platform/desktop build:extension arrancador`
- `bun run --cwd platform/desktop build:extension delphi`
- `bun test products/eden/tests/vimMotions.test.ts`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-internal-type-exports/desloppify-after.json`

## Results

- References check: PASS.
  - Scoped type symbols are only used inside their defining files.
  - Externally imported APIs remain exported: `VIM_MOTION_GROUPS`,
    `LaunchResult`, `ScanResult`, `RawgGame`, `SqobaBackup`,
    `createDictationConfig`, `DictationConfigKey`, and settings option
    constants.
- `typecheck`: PASS.
- `build:extension eden`: PASS.
- `build:extension arrancador`: PASS.
- `build:extension delphi`: PASS.
- `vimMotions.test.ts`: initially exposed a real missing `:zen off` item; fixed
  in `VIM_MOTION_GROUPS`, then PASS, 1 test.
- Baseline scan: score 0, 472 findings; severity critical 0, high 271,
  medium 130, low 71.
- Final scan: score 0, 460 findings; severity critical 0, high 259,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 12 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Scoped type-only exports are removed.
- AC3: PASS. Public APIs remain exported.
- AC4: PASS. Relevant checks passed.
