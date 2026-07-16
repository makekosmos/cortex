# Evidence

Verified against the final worktree at `2026-07-15T15:23:27Z`.

## AC1 — PASS

- `cargo test -p kepler-backend calculator`
- Result: 3 tests passed. The RPC test proves `calculator.evaluate` returns `"1440"` for a calculation and `null` for ordinary search text. The evaluator tests also prove a currency query returns `null`; input is bounded to 256 characters and 50 ms and uses `fend-core` preview mode without an exchange-rate/network handler.

## AC2 — PASS

- `cargo test -p kepler-backend calculator`
- Result: representative arithmetic, percentage, unit-conversion, and date-arithmetic assertions passed.

## AC3 — PASS

- `bunx playwright test tests/e2e/launcher-calculator.spec.ts --config playwright.config.ts`
- Result: 1 test passed against the real Electron launcher and Rust backend. It verified the unit conversion, replaced it with the newer arithmetic query, and asserted the calculator row is first and selected. The renderer additionally guards every asynchronous response with `calculatorRun`.

## AC4 — PASS

- `bunx playwright test tests/e2e/launcher-calculator.spec.ts --config playwright.config.ts`
- Result: Enter on the selected result changed the system clipboard sentinel to `1440`; the calculator branch returns before command-bus invocation.

## AC5 — PASS

- `cargo fmt --all --check` — PASS.
- `cargo test -p kepler-backend calculator` — 3 passed.
- `bun run desktop:typecheck` — PASS.
- `bunx oxfmt --check platform/desktop/src/views/LauncherView.vue tests/e2e/launcher-calculator.spec.ts RAYCAST-PARITY.md .agent/tasks/2026-07-15-launcher-calculator/spec.md` — PASS.
- `bunx playwright test tests/e2e/launcher-calculator.spec.ts --config playwright.config.ts` — 1 passed.
- `git diff --check` — PASS.
- Visual artifact inspected: `.tmp/visual/2026-07-15-launcher-calculator/launcher-calculation.png`. The result, calculator icon, selected state, `Результат` label, and `Копировать ↵` footer are visible without clipping.
