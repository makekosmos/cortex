# Evidence

Verified at: 2026-07-19T20:08:15Z

## AC1 — PASS

- `bunx playwright test tests/e2e/coder.spec.ts` — 1 passed; проверяет, что computed cursor вкладки LeetCode не равен `pointer`.
- `node .tmp/visual/2026-07-19-coder-codewars-kyu/verify.mjs` — PASS для обеих вкладок.

## AC2 — PASS

- E2E проверяет отсутствие текста «Решено» в `.difficulty-gauge` и размер центрального числа больше 16 px.
- Визуально проверен `leetcode-desktop.png`: слева нет текстового блока, в центре gauge показано крупное `78`.

## AC3 — PASS

- `cargo test -p kepler-backend --lib integrations::tests::codewars` — 4 passed.
- Rust-тесты подтверждают сохранение `rank.name` в Codewars completion и одноразовый backfill по отсутствию ключа `rank` для текущего username.
- `bun run ark:guard:writes` — `ARK write boundary guard passed`; запись остаётся на существующем `upsert_object` path.
- Официальный контракт проверен по `https://dev.codewars.com/`: completed list не содержит rank, `/api/v1/code-challenges/{challenge}` возвращает объект `rank`.

## AC4 — PASS

- `bun test platform/desktop/src/coder/useCoderStats.test.ts` — 6 passed; распределение сортируется `8 kyu` → `1 kyu` и исключает не-kyu значения.
- E2E проверяет панель «Решённые kata по kyu» и количество `1` для `6 kyu`.
- Визуальный mock проверяет порядок/количества `8/6/4 kyu` = `1/2/1`; `codewars-desktop.png` просмотрен.

## AC5 — PASS

- `bun run desktop:typecheck` — PASS.
- `bun run desktop:build` — PASS.
- `bunx playwright test tests/e2e/coder.spec.ts` — 1 passed.
- `git diff --check` — PASS.
- Скриншоты: `.tmp/visual/2026-07-19-coder-codewars-kyu/leetcode-desktop.png`, `.tmp/visual/2026-07-19-coder-codewars-kyu/codewars-desktop.png`.

## Non-gating diagnostic

`cargo clippy -p kepler-backend --lib -- -D warnings` остановился на шести ранее существовавших warnings (`dictation/*` и старые строки `integrations.rs`), не на изменённых строках. Targeted Rust tests проходят.

Live sync с пользовательским Codewars-профилем не запускался: контракт проверен по официальной документации, network mapping покрыт компиляцией и targeted mapping/backfill tests.
