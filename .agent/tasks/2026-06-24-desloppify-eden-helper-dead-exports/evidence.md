# Evidence

## Commands

- `rg EdenApi|setEdenLastEntry|openEdenNote|twoTaskRefsDoc|taskRefParagraphTaskRefDoc tests/e2e/helpers tests/e2e platform products packages`
- `KOSMOS_HEADLESS=1 KOSMOS_TEST_MODE=1 bunx playwright test tests/e2e/eden.spec.ts tests/e2e/eden-task-enter.spec.ts`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension eden`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-eden-helper-dead-exports/desloppify-after.json`

## Results

- References check: PASS. Removed export names had no cross-file references;
  remaining hits are local-only uses inside `tests/e2e/helpers/eden.ts`.
- Focused Eden e2e: PARTIAL/FAIL, raw log at
  `raw/focused-eden-e2e.log`.
  - 3 tests passed.
  - Failures are existing Eden autosave/journal/task assertions, not missing
    helper symbols: removed symbols are not imported by the failing specs.
- `typecheck`: PASS.
- `build:extension eden`: PASS.
- Baseline scan: score 0, 513 findings; severity critical 0, high 311,
  medium 131, low 71.
- Final scan: score 0, 507 findings; severity critical 0, high 306,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 5 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Scoped `DEAD_EXPORT` findings are removed after `rg` reference
  checks.
- AC3: PASS with documented unrelated e2e blocker. Remaining helper code
  typechecks and Eden extension build passes; broad focused Eden specs fail in
  autosave/journal assertions unrelated to removed helper exports.
- AC4: PASS. This slice does not touch `launchKepler`, `KOSMOS_HEADLESS`,
  `KOSMOS_TEST_MODE`, or `KOSMOS_DATA_DIR`.

## Deferred

- Investigate Eden autosave/journal e2e failures separately; they are outside
  this dead-export cleanup.
