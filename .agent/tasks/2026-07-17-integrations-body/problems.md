# Verification problems

## P1 — hidden Electron window screenshots timed out

- First run: `playwright test tests/e2e/integrations-body.spec.ts`
- Result: both functional flows reached their final screenshot, then Playwright `page.screenshot()`
  timed out because Kosmos E2E windows are intentionally created with `show: false`.
- Product behavior and assertions before the screenshot passed.
- Smallest fix: capture the hidden window through Electron `webContents.capturePage()` after an
  invalidate, matching the established launcher visual-test pattern.
- Reverification: `PASS` — both E2E flows passed, and the refreshed screenshots show the
  integrations page plus both body subpages.

## P2 — unrelated Rust shared-state/timing tests failed under parallel runs

- The first full-library attempt accidentally overlapped two identical processes; a later normal
  parallel run still exposed the same existing shared-environment/timing family.
- Failures were confined to existing dictation mock/env tests and the agents approval deadline.
- Every reported test passed immediately when rerun alone (`1/1` each).
- Smallest fix: no product code change; use the repository suite's deterministic serial mode for
  the fresh full-library verdict.
- Reverification: `PASS` — `451 passed, 5 ignored` with `--test-threads=1`; targeted integrations
  remain `4/4 PASS`.

## P3 — unrelated Eden journal assertion failed in a broad command-bus run

- The additional command architecture suite passed the command-registration coverage relevant
  to this task, including the new built-in command list.
- One existing `eden:note:open-today` case failed to observe its journal object; the other eight
  selected desktop tests, including calculator, integrations/body, and repeated dashboard open,
  passed.
- No task code participates in Eden journal creation.
- Isolated rerun reproduced the same Eden-only failure, so it is not a load-order flake from this
  task. Fixing Eden journal creation would be an unrelated product change.
- Task verdict: out-of-scope baseline issue. Relevant command registration passed, as did direct
  `kosmos:body` invocation and repeated dashboard opening.
