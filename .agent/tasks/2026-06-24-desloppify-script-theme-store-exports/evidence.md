# Evidence

## Commands

- `rg readVersions|writeVersions|getVersion|useTheme\\(|objectTypesLoading|chooseUsageIconRef|useTodoStore|store/todos platform/desktop/scripts platform/desktop/src products/eden/src products/delphi/src products/delphi/tests tests`
- `node platform/desktop/scripts/release-version.mjs get win`
- `bun platform/desktop/src/dashboard/store.regression.mjs`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension eden`
- `bun run --cwd platform/desktop build:extension delphi`
- `bun run --cwd platform/desktop build:js:shell`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-script-theme-store-exports/desloppify-after.json`

## Results

- References check: PASS.
  - `readVersions` and `writeVersions` are internal to release-version CLI/API;
    `getVersion` remains exported for `build-desktop.mjs`.
  - Eden imports `useTheme.ts` for side effects and does not call `useTheme()`.
  - Delphi imports `useTodoStore` as a named export everywhere; no default
    import exists.
  - `objectTypesLoading` is internal to Dashboard store.
  - `chooseUsageIconRef` is a confirmed false positive: it is imported by
    `platform/desktop/src/dashboard/store.regression.mjs` and remains exported.
- Distribution docs check: PASS. No version, release channel, publishing, or
  wire-format changes were made.
- `release-version get win`: PASS.
- Dashboard regression: PASS.
- `typecheck`: PASS.
- `build:extension eden`: PASS.
- `build:extension delphi`: PASS.
- `build:js:shell`: PASS with existing Vite warnings about chunk size and
  `inlineDynamicImports`.
- Baseline scan: score 0, 456 findings; severity critical 0, high 255,
  medium 130, low 71.
- Final scan: score 0, 451 findings; severity critical 0, high 250,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 6 -> 1, where the remaining item is the
  `chooseUsageIconRef` false positive.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Confirmed unused scoped exports are removed; the remaining scoped
  finding is documented as false positive.
- AC3: PASS. Runtime behavior is covered by CLI/regression/build checks.
- AC4: PASS. Relevant checks passed.
