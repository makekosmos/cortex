# Evidence

## Commands

- `rg normalizeApiUrl|normalizePassphrase|createChecklistItem|setSidebarHidden|useSidebarState|absoluteUrl products/delphi/src products/delphi/tests platform tests packages`
- `rg cn\\( products/delphi/src products/delphi/tests`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension delphi`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-delphi-small-helper-dead-exports/desloppify-after.json`

## Results

- References check: PASS.
  - `absoluteUrl` and `setSidebarHidden` had no cross-file references.
  - `createChecklistItem` was only used internally by `addChecklistItem`.
  - `normalizePassphrase` is a false positive and remains unchanged because it
    is used by `products/delphi/src/services/api/client.ts`.
  - Active helpers `cn`, `useSidebarState`, and checklist mutation helpers remain
    used.
- `typecheck`: PASS.
- `build:extension delphi`: PASS.
- Baseline scan: score 0, 504 findings; severity critical 0, high 303,
  medium 130, low 71.
- Final scan: score 0, 501 findings; severity critical 0, high 300,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 3 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Confirmed dead helper exports are removed.
- AC3: PASS. Active helpers remain exported and typechecked.
- AC4: PASS. Relevant TypeScript checks passed.

## False Positives

- `products/delphi/src/helpers/normalize.ts::normalizePassphrase` remains a
  `DEAD_EXPORT` false positive in `desloppify`; it is imported and used by
  `services/api/client.ts`.
