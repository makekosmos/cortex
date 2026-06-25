# Evidence

## Iteration 1: focus widget e2e cleanup

Commands:

- `bun run --cwd platform/desktop build:js:shell`
- `KOSMOS_HEADLESS=1 KOSMOS_TEST_MODE=1 bunx playwright test tests/e2e/focus-widget-controls.spec.ts`
- `bunx desloppify scan --json . > .tmp/desloppify-after-focus-widget.json`

Results:

- `build:js:shell`: PASS
- Focus widget e2e: PASS, 7 passed
- `tests/e2e/focus-widget-controls.spec.ts` findings: 31 -> 0
- Full scan findings: 552 -> 521
- Medium findings: 161 -> 130

## Iteration 2: extension manifest extraction

Commands:

- `bun run --cwd platform/desktop build:js:shell`
- `KOSMOS_HEADLESS=1 KOSMOS_TEST_MODE=1 bunx playwright test tests/e2e/extension-ark-bridge.spec.ts tests/e2e/extension-permissions.spec.ts`
- `KOSMOS_HEADLESS=1 KOSMOS_TEST_MODE=1 bunx playwright test --config platform/desktop/playwright.config.ts extension-api-compat.spec.ts`
- `bunx desloppify scan --json . > .tmp/desloppify-after-extension-manifest.json`

Results:

- `build:js:shell`: PASS
- Extension ARK bridge + permissions e2e: PASS, 4 passed
- `extension-api-compat.spec.ts`: PARTIAL
  - incompatible extension window test: PASS
  - launch-only sanity test: FAIL before app attach with Electron process exit code 0
  - The failing test starts without isolated `KOSMOS_DATA_DIR` and uses a shared
    userDataDir; it does not exercise the extracted manifest logic.
- `extension-host.ts` line count: 1430 physical lines
- Final finding for `extension-host.ts` in this scan: no `GOD_FILE` critical;
  remaining findings are `LARGE_FILE` high and `DEAD_FILE` high.

## Iteration 3: main.ts IPC extraction

Files changed:

- `platform/desktop/electron/main.ts`
- `platform/desktop/electron/main-data-ipc.ts`
- `platform/desktop/electron/main-crashes-ipc.ts`
- `platform/desktop/electron/main-shell-ipc.ts`

Commands:

- `bun run --cwd platform/desktop build:js:shell`
- `KOSMOS_HEADLESS=1 KOSMOS_TEST_MODE=1 bunx playwright test tests/e2e/focus-widget-controls.spec.ts`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-23-desloppify-cleanup/desloppify-final.json`

Results:

- `build:js:shell`: PASS
- Focus widget e2e: PASS, 7 passed
- Final scan: score 0, 532 findings
- Severity: critical 0, high 330, medium 131, low 71
- Categories: defensive-programming 55, complexity 57, ai-slop 19,
  inconsistency 15, test-quality 50, runtime-validation 15,
  naming-semantics 4, async-correctness 8, dead-code 309
- `platform/desktop/electron/main.ts`: `GOD_FILE` critical removed; final
  finding is `LARGE_FILE` high at 822 LOC.
- `platform/desktop/electron/extension-host.ts`: no `GOD_FILE` critical;
  remaining `LARGE_FILE` high.
- `tests/e2e/focus-widget-controls.spec.ts`: 0 findings in final scope scan.
- Remaining `DEAD_FILE` findings for Electron entry/helper modules are treated
  as audit false positives: the modules are imported by the built Electron
  entry and `build:js:shell` passes.
- `main-commands.ts` `FAKE_LOADING_DELAY` is a false positive: the `setTimeout`
  is bounded polling for dynamic extension command registration, not simulated
  production latency.
