# Evidence — daily currency rates

Verified at: 2026-07-15T16:38:48Z

## AC1 — daily CBR rates only for currency-like queries

**PASS.** `cargo test -p kepler-backend calculator --lib` passed. The HTTP mock test evaluates two currency queries against one cache directory, observes exactly one request, and confirms `10 km to miles` performs no rate request.

## AC2 — durable cache and stale fallback

**PASS.** The same Rust suite passed the fresh-cache and stale-cache tests. The stale test persists an expired snapshot, points refresh at a refused local port, and still loads USD from the cached snapshot.

## AC3 — nominal-aware mathematically correct conversion

**PASS.** The parser test passed for RUB, USD, and an AMD rate quoted per 10 units. It verifies `100 USD to RUB` produces exactly `9000.00 RUB` through `fend-core`.

## AC4 — existing launcher behavior

**PASS.** `launcher-calculator.spec.ts` passed against the isolated rebuilt backend. It covers arithmetic, units, stable card updates, daily cached currency conversion, selection, screenshot rendering, Enter, and system clipboard copy.

## AC5 — final checks

**PASS.** The following commands passed against the final implementation:

- `cargo fmt --check`
- `bunx oxfmt --check tests/e2e/launcher-calculator.spec.ts RAYCAST-PARITY.md`
- `cargo test -p kepler-backend calculator --lib`
- `cargo test -p kepler-backend calculator::tests::live_cbr_rates_smoke --lib -- --ignored`
- `cargo build -p kepler-backend --bin kepler-backend --target-dir .tmp/cargo-calculator-currency`
- `bun run --cwd platform/desktop typecheck`
- `KEPLER_BACKEND_EXE=.tmp/cargo-calculator-currency/debug/kepler-backend.exe bunx playwright test tests/e2e/launcher-calculator.spec.ts --config playwright.config.ts`
- `git diff --check`

Visual artifact: `.tmp/visual/2026-07-15-launcher-calculator/launcher-currency.png`.

The default `target/debug/kepler-backend.exe` was intentionally not replaced because the user's active Electron dev process owns it; the verifier used the supported `KEPLER_BACKEND_EXE` override with an isolated target instead.
