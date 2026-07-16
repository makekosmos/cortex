# Evidence — Russian currency input

Verified at: 2026-07-15T16:55:05Z

## AC1 — Russian normalization

**PASS.** `cargo test -p kepler-backend calculator --lib` verifies that `900 долларов в рублях` normalizes to `900 USD to RUB` and evaluates to `81000.00 RUB` with fixed cached rates.

## AC2 — separate compact display expression

**PASS.** Rust tests verify `900 долларов в рублях → 900 USD`, `100 USD to RUB → 100 USD`, and non-currency `1200 * 1.2 → 1200 * 1.2`. The RPC test confirms the separate `expression` field for ordinary calculations.

## AC3 — launcher rendering and atomic updates

**PASS.** `launcher-calculator.spec.ts` verifies both ISO and Russian inputs, source-only expression rendering, the existing no-removal MutationObserver regression, and clipboard behavior.

## AC4 — final checks

**PASS.** Rust tests, isolated backend build, desktop typecheck/build, targeted Electron E2E, formatting, `git diff --check`, and visual inspection passed.

Visual artifact: `.tmp/visual/2026-07-15-launcher-calculator/launcher-currency.png`.
