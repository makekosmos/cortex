# Evidence

## Commands

- `rg SHELL_ROOT|E2E_ROOT|waitForCommandRegistered|getTestStats|POMODORO_STATE_FILENAME|readPomodoroStateFile|writePomodoroStateFile|SessionConfigShape tests/e2e`
- `KOSMOS_HEADLESS=1 KOSMOS_TEST_MODE=1 bunx playwright test tests/e2e/focus-widget-controls.spec.ts tests/e2e/commands-architecture.spec.ts`
- `bun run --cwd platform/desktop build:js:shell`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-e2e-helper-exports/desloppify-after.json`

## Results

- References check: removed exports had no cross-file references; remaining hits
  are internal uses inside the helper files.
- Focused e2e: PASS, 12 passed.
- `build:js:shell`: PASS.
- Baseline scan: score 0, 532 findings; severity critical 0, high 330,
  medium 131, low 71.
- Final scan: score 0, 523 findings; severity critical 0, high 321,
  medium 131, low 71.
- Scoped `DEAD_EXPORT`: 9 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Scoped `DEAD_EXPORT` findings are removed after `rg` reference
  checks.
- AC3: PASS. Focused headless e2e specs passed.
- AC4: PASS. `launchKepler` still sets isolated `KOSMOS_DATA_DIR`,
  `KOSMOS_HEADLESS=1`, and `KOSMOS_TEST_MODE=1`.

## False Positives Deferred

- Repo-wide `DEAD_FILE` remains noisy: test files, Electron entry modules, and
  Vite/app entrypoints are reported as unused despite being runtime/test
  entrypoints.
