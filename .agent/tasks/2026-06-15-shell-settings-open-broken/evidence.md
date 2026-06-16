# Evidence

## Verification Summary

- `KOSMOS_HEADLESS=1 bunx playwright test --config platform/desktop/playwright.config.ts platform/desktop/e2e/tests/e2e/headless-window-repeat-open.spec.ts` -> PASS
- Headed Electron visual verification -> PASS:
  - Session launched with `KEPLER_INSTANCE=dev-visual`, `KOSMOS_DATA_DIR=.tmp/settings-open-verify-data`
  - `KOSMOS_HEADLESS` and `KOSMOS_TEST_MODE` were not set
  - Command executed in the headed session: `window.kepler.commands.invoke("settings:open")`
  - Log: `.tmp/logs/settings-open-verify.log`
  - Screenshots:
    - `.tmp/visual/2026-06-16-settings-open-verify/settings-after-first-command.png`
    - `.tmp/visual/2026-06-16-settings-open-verify/settings-after-refocus-from-minimized.png`
    - `.tmp/visual/2026-06-16-settings-open-verify/settings-after-refocus-from-hidden.png`

## Acceptance Criteria

### AC1

Status: PASS

Evidence:

- Headed Electron visual verification confirmed that `window.kepler.commands.invoke("settings:open")` opens Settings visible and focused in a real desktop session.
- The same headed session confirmed Settings restores and focuses after minimize, and shows and focuses after hide.
- Evidence files:
  - `.tmp/logs/settings-open-verify.log`
  - `.tmp/visual/2026-06-16-settings-open-verify/settings-after-first-command.png`
  - `.tmp/visual/2026-06-16-settings-open-verify/settings-after-refocus-from-minimized.png`
  - `.tmp/visual/2026-06-16-settings-open-verify/settings-after-refocus-from-hidden.png`

### AC2

Status: PASS

Evidence:

- `platform/desktop/e2e/tests/e2e/headless-window-repeat-open.spec.ts`

### AC3

Status: PASS

Evidence:

- `platform/desktop/e2e/tests/e2e/headless-window-repeat-open.spec.ts`

### AC4

Status: PASS

Evidence:

- `platform/desktop/e2e/tests/e2e/headless-window-repeat-open.spec.ts`
- `.agent/tasks/2026-06-15-shell-settings-open-broken/raw/playwright-headless-window-repeat-open.log`

### AC5

Status: PASS

Evidence:

- Only the focused headless repeat-open e2e coverage was extended; no unrelated shell or ARK changes were made in this pass.

## Notes

- The new test now exercises both `kepler:commands:invoke('settings:open')` and `window.kepler.settings.open()` while keeping the one-settings-window invariant in headless mode.
- Residual visual risk was closed by the headed Electron verification using `KEPLER_INSTANCE=dev-visual` and `KOSMOS_DATA_DIR=.tmp/settings-open-verify-data`, with no `KOSMOS_HEADLESS`/`KOSMOS_TEST_MODE` set.
