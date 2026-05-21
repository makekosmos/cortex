---
title: Эксперименты
description: Журнал измерений — гипотеза → бейзлайн → замер → verdict. Объективные данные складируются здесь.
---

# Эксперименты

Этот журнал — там где мы складываем **объективные данные** о принятых tooling- и архитектурных решениях. Каждая запись:

1. **Гипотеза** — что должно дать изменение.
2. **Метрика** — что меряем, в чём.
3. **Бейзлайн** — 3-5 замеров до.
4. **After** — 3-5 замеров после.
5. **Verdict** — гипотеза подтверждена / опровергнута / в шуме.
6. **Raw** — ссылка на сырые логи в `.agent/experiments/<date-slug>/`.

Зачем: чтобы не пересказывать чужие бенчмарки как свои, не «50-100× быстрее» без замеров, и чтобы при сомнении «правда ли это даёт что обещано» можно было открыть таблицу и увидеть число.

См. [feedback-baseline-before-experiment](https://github.com/ksanrse/kepler/blob/main/CLAUDE.md) в правилах агента.

---

## 2026-05-19 — Tooling pass

Single batch — 4 эксперимента в одну сессию: cargo-nextest, cargo-shear, Pinia Colada install, Vitest browser. Raw: [`.agent/experiments/2026-05-19-tooling-pass/`](https://github.com/ksanrse/kepler/tree/main/.agent/experiments/2026-05-19-tooling-pass).

### E1 — cargo-nextest заменяет `cargo test`

|              |                                                                                                                                                                |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Гипотеза** | Параллельный test runner быстрее `cargo test` за счёт subprocess isolation + scheduler. Ожидаем 20-50% wall-clock.                                             |
| **Метрика**  | wall-clock `cargo test --workspace` vs `cargo nextest run --workspace` (warm cache, 5 runs медиана).                                                           |
| **Caveat**   | `kepler-focus-helper`/`kepler-focus-svc` исключены — их `embed_manifest` инжектит `requireAdministrator` в test binaries, падают с UAC. Pre-existing проблема. |

| Run             | `cargo test` (s) | `cargo nextest` (s) |
| --------------- | ---------------- | ------------------- |
| 1 (cold)        | 84.871           | 12.625              |
| 2               | 8.588            | 8.100               |
| 3               | 8.438            | 8.068               |
| 4               | 8.350            | 8.091               |
| 5               | 8.122            | 8.204               |
| **median warm** | **8.394**        | **8.096**           |

**Δ: −0.30s (−3.5%) — в пределах шума.**

**Verdict**: гипотеза «20-50% быстрее» **опровергнута**. Тесты CPU-light (большинство <100ms), bottleneck в test-binary launch overhead, не в исполнении. Нет смысла принимать ради perf.

**Adopt anyway**: за **non-perf wins** — progress-bar, per-test subprocess isolation, `--retries N` для flaky тестов, JUnit XML output. Если e2e начнёт флейкать — retries пригодятся. Заменён `cargo test` → `cargo nextest run` в `lefthook.yml::pre-push`.

raw: [`raw/baseline-cargo-test.txt`](https://github.com/ksanrse/kepler/blob/main/.agent/experiments/2026-05-19-tooling-pass/raw/baseline-cargo-test.txt), [`raw/after-cargo-nextest.txt`](https://github.com/ksanrse/kepler/blob/main/.agent/experiments/2026-05-19-tooling-pass/raw/after-cargo-nextest.txt).

### E2 — cargo-shear для orphan deps

|              |                                                                                                       |
| ------------ | ----------------------------------------------------------------------------------------------------- |
| **Гипотеза** | Удалит N unused deps. Cold-build может ускориться, эффект скорее всего шумовой.                       |
| **Метрика**  | (1) кол-во удалённых deps. (2) `cargo clean && cargo build --workspace --release` wall-clock, 3 runs. |

**Найдено и удалено**: 6 orphan deps в 4 `Cargo.toml`'ах (−8 строк).

| Где                         | Что удалено                                        |
| --------------------------- | -------------------------------------------------- |
| `crates/ark-core/rust`      | `uuid`                                             |
| `services/ark-relay-server` | `axum`, `serde`, `uuid` (legacy после refactor'ов) |
| `services/kepler-backend`   | `tokio-test` (dev-dep, никогда не использовался)   |
| `services/kepler-watcher`   | `chrono`                                           |

| Run        | Baseline cold-build (s) | After cold-build (s) |
| ---------- | ----------------------- | -------------------- |
| 1          | 210.0                   | 220.4                |
| 2          | 196.2                   | 188.5                |
| 3          | 162.2                   | 184.6                |
| **median** | **196.2**               | **188.5**            |

**Δ: −7.7s (−3.9%) — в пределах шума** (variance baseline ±25%).

**Verdict**: perf-выигрыш нулевой как и предсказано — orphan deps в Cargo workspace **не компилируются** если не используются (Rust dependency resolver видит что нет use-цепочки и пропускает). Удаление — гигиена, не perf.

**Adopt**: за чистоту `Cargo.toml` (легче читать какие deps реально нужны) + pre-push hook fail если новые orphan'ы появятся.

raw: [`raw/baseline-cargo-build-cold.txt`](https://github.com/ksanrse/kepler/blob/main/.agent/experiments/2026-05-19-tooling-pass/raw/baseline-cargo-build-cold.txt), [`raw/after-cargo-build-cold.txt`](https://github.com/ksanrse/kepler/blob/main/.agent/experiments/2026-05-19-tooling-pass/raw/after-cargo-build-cold.txt), [`raw/shear-fix.txt`](https://github.com/ksanrse/kepler/blob/main/.agent/experiments/2026-05-19-tooling-pass/raw/shear-fix.txt).

### E3 — Pinia Colada install (без миграции)

|              |                                                                                                                                               |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **Гипотеза** | Install drop-in, bundle вырастет на 5-10KB gzip.                                                                                              |
| **Метрика**  | (1) bundle Eden `index.js` raw + gzip. (2) `wc -l store/eden.ts` (baseline для будущей миграции). (3) e2e wall-clock (baseline для Phase 14). |

| Метрика                        | Baseline                   | After install              | Δ                        |
| ------------------------------ | -------------------------- | -------------------------- | ------------------------ |
| `eden.ts` LoC                  | 686                        | 686                        | 0 (миграция не делалась) |
| Bundle `index.js` raw          | 358,873 bytes              | 363,421 bytes              | +4,548 (+1.3%)           |
| **Bundle `index.js` gzip**     | **113,089 bytes (110 KB)** | **114,819 bytes (112 KB)** | **+1,730 (+1.5%)**       |
| Eden e2e median (3 clean runs) | 89s (9/9 tests)            | —                          | (после миграции замерим) |

**Δ bundle: +1.7 KB gzip — лучше прогноза (5-10KB).**

**Verdict**: install дешевле ожидаемого. Drop-in, никакого behavior change, typecheck + build extensions зелёные. Полная миграция queries/mutations отложена до Phase 14.

raw: bundle до — see git history, после — `extensions/eden/dist/assets/index-DkfQJbmO.js`. E2e baseline: [`raw/baseline-eden-e2e.txt`](https://github.com/ksanrse/kepler/blob/main/.agent/experiments/2026-05-19-tooling-pass/raw/baseline-eden-e2e.txt).

### E4 — Vitest browser mode (capability add)

|             |                                                                                                                                                                          |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Тип**     | Не perf-эксперимент — capability add. Был только bun:test (pure JS) и Playwright e2e (full Electron). Vitest browser — middle layer для Vue components на real Chromium. |
| **Метрика** | sanity check — 5 pilot тестов работают.                                                                                                                                  |

**Setup**: `vitest@4.1.6` + `@vitest/browser-playwright@4.1.6` + `vitest-browser-vue@2.1.0` + `@vitejs/plugin-vue@6.0.7`. Конфиг в `extensions/eden/vitest.config.ts`. Запуск: `bun run --cwd extensions/eden test:vue`.

Pilot test `tests/components/CharCounter.spec.ts` — 5/5 passing на real Chromium за 1.9s.

**Verdict**: infra работает, расширяемо. Layer cake для Eden теперь: `bun:test` (pure JS) → `vitest browser` (component) → `playwright` (full e2e). Через 2 коммита добавлено ещё 8 тестов на bug-prone места (journal title readonly, untitled flag) — итого 13 component тестов.

---

## Сводка по решениям

| Эксперимент             | Гипотеза                | Реальный результат          | Adopt                        |
| ----------------------- | ----------------------- | --------------------------- | ---------------------------- |
| E1 cargo-nextest        | 20-50% perf-win         | −3.5% (шум)                 | ✅ за UX/retries, не за perf |
| E2 cargo-shear          | hygiene + возможно perf | 6 deps удалено, perf в шуме | ✅ за hygiene                |
| E3 Pinia Colada install | +5-10 KB gzip           | +1.7 KB gzip                | ✅ дешевле прогноза          |
| E4 Vitest browser       | n/a (capability add)    | 13 тестов работают          | ✅ infra add                 |

**Главный урок 2026-05-19**: гипотеза «20-50% быстрее» по nextest опровергнута — я зря продавал её до замеров. Хорошо что мерили: без замеров я бы думал что получил большой выигрыш, а получил гигиенический + UX wins. Без замеров проектные решения = вкусовщина под маской объективности.
