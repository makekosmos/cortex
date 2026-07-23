# Evidence

Verified at: 2026-07-19T20:31:51Z

## AC1 — PASS

- Deterministic Playwright visual check found 3 Codewars `.gauge-segment` paths for `8/6/4 kyu` and central total `4`.
- Inspected screenshot: `.tmp/visual/2026-07-19-coder-codewars-kyu-chart/codewars-desktop.png`.

## AC2 — PASS

- Codewars gauge legend is `8 kyu: 1`, `6 kyu: 2`, `4 kyu: 1` in descending kyu order.
- Electron E2E asserts the kyu legend/count and absence of the old «Решённые kata по kyu» breakdown card.

## AC3 — PASS

- `cargo build --manifest-path Cargo.toml --bin kepler-backend --target-dir .tmp/cargo-coder-chart` — PASS; 345 crates compiled.
- The verified workspace backend binary replaced the locked old dev binary and the exact Kosmos Vite/Electron/backend process tree was restarted.
- Real `Kosmos-dev` ARK check via `integrations.sync_now` returned `before: total 102, ranked 102, missing 0; imported 6; after: total 102, ranked 102, missing 0`.
- `cargo test -p kepler-backend --lib integrations::tests::codewars` — 4 passed.

## AC4 — PASS

- Inspected LeetCode screenshot keeps Easy/Medium/Hard legend including `0/953`, central `78`, and no visible «Уровень алгоритмов» / «Решено» inside the gauge.
- Electron E2E confirms the central value remains larger than 16 px.

## AC5 — PASS

- `bun test platform/desktop/src/coder/useCoderStats.test.ts` — 6 passed.
- `bun run desktop:typecheck` — PASS.
- `bun run desktop:build` — PASS.
- `bun run ark:guard:writes` — PASS.
- `bunx playwright test tests/e2e/coder.spec.ts` — 1 passed.
- `node .tmp/visual/2026-07-19-coder-codewars-kyu/verify.mjs` — PASS.
- `git diff --check` — PASS.

No unverified user-visible acceptance criterion remains.
