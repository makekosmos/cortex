# Experiments — tooling pass 2026-05-19

Введение nextest / shear / Pinia Colada. Каждый — как изолированный эксперимент с гипотезой, метрикой, и замерами 3-5 раз до/после.

## Структура

- `baseline.md` (этот файл) — гипотезы + raw-метрики.
- `raw/` — сырые логи `time` / `cargo --timings` / `ls -la dist/` и т.п.
- `results.md` — финальная сводка с verdict'ом по каждому эксперименту.

## E1 — cargo-nextest замена `cargo test`

**Гипотеза:** parallel test runner быстрее `cargo test` за счёт subprocess-isolation + scheduler, на нашем объёме тестов ожидаем выигрыш 20-50% wall-clock.

**Метрика:** wall-clock `cargo test --workspace --quiet` vs `cargo nextest run --workspace`.

**Замеры:** 5 × до (warm cache), 5 × после.

**Caveat:** `kepler-focus-helper` и `kepler-focus-svc` исключены через `--exclude`. Их `build.rs` через `embed_manifest` инжектит `requireAdministrator` в test-binaries тоже → `cargo test` падает с UAC error 740. Pre-existing проблема, не часть эксперимента; обоим runners одинаково.

### Baseline (`cargo test --workspace --exclude kepler-focus-{helper,svc} --quiet`)

| Run | wall-clock (s) | прошло тестов |
|---|---|---|
| 1 (cold) | 84.871 | — (compile) |
| 2 | 8.588 | 314 |
| 3 | 8.438 | 314 |
| 4 | 8.350 | 314 |
| 5 | 8.122 | 314 |
| **median (warm 2-5)** | **8.394** | |

raw: `raw/baseline-cargo-test.txt`

### After (`cargo nextest run --workspace --exclude kepler-focus-{helper,svc}`)

| Run | wall-clock (s) | прошло тестов | nextest inner (s) |
|---|---|---|---|
| 1 (warm-ish) | 12.625 | 314 | 6.168 |
| 2 | 8.100 | 314 | 6.631 |
| 3 | 8.068 | 314 | 6.631 |
| 4 | 8.091 | 314 | 6.745 |
| 5 | 8.204 | 314 | 6.791 |
| **median (warm 2-5)** | **8.096** | | 6.700 |

raw: `raw/after-cargo-nextest.txt`

### Verdict E1

| Метрика | Baseline | After | Δ |
|---|---|---|---|
| wall-clock warm median | 8.394s | 8.096s | **−0.30s (−3.5%)** — в пределах шума |
| тесты прошли | 314 | 314 | — |

**Реальный выигрыш не там где ожидался.** Гипотеза «20-50% быстрее» опровергнута на нашем масштабе тестов — наши тесты CPU-light (большинство <100ms), bottleneck в test-binary launch overhead, а не в исполнении. Параллелизация nextest'а раскрывается на медленных тестах или массиве крупных suite'ов.

**Что nextest всё-таки даёт** (qualitative, не в этих метриках):
- Прогресс-бар во время прогона (видно сколько осталось).
- Per-test изоляция через subprocess — каждый тест в своём процессе, panic'и одного не убивают остальных.
- JUnit XML output (для CI integration в будущем).
- `--retries N` для flaky тестов.
- Лучшая визуализация failures (имена и backtraces в финале).

**Решение:** оставляем nextest. Performance gain маргинальный, но UX-улучшения и потенциал для CI (особенно retries для e2e flakes) оправдывают переход. Сменю `cargo test` → `cargo nextest run` в `lefthook.yml::pre-push`.

---

## E2 — cargo-shear cleanup

**Гипотеза:** найдёт N orphan-deps в Cargo workspace; cold full-build `cargo build --workspace --release` может ускориться (но эффект скорее всего шумовой ±1-2s).

**Метрика:** (1) кол-во удалённых deps. (2) cold-build time.

**Замеры:** 3 × до, 3 × после (cold = `cargo clean` между).

### Baseline (`cargo clean && cargo build --workspace --release`)

| Run | wall-clock (s) |
|---|---|
| 1 | 210.0 (3m30s) |
| 2 | 196.2 (3m16s) |
| 3 | 162.2 (2m42s) |
| **median** | **196.2** |

raw: `raw/baseline-cargo-build-cold.txt`

### Cleanup pass (`cargo shear --fix`)

- **Удалено: 6 unused deps** в 4 Cargo.toml файлах:
  - `crates/ark-core/rust/Cargo.toml`: `uuid`
  - `services/ark-relay-server/Cargo.toml`: `axum`, `serde`, `uuid`
  - `services/kepler-backend/Cargo.toml`: `tokio-test` (был указан как dev-dep, не использовался)
  - `services/kepler-watcher/Cargo.toml`: `chrono`
- **Затронуто Cargo.toml**: 4 файла, −8 строк.
- raw: `raw/shear-dryrun.txt`, `raw/shear-fix.txt`

Verify post-cleanup: `cargo check --workspace` зелёный (48s); `cargo nextest run --workspace --exclude focus-*` — 314/314 passed.

### After (`cargo clean && cargo build --workspace --release`)

| Run | wall-clock (s) |
|---|---|
| 1 | 220.4 (3m40s) |
| 2 | 188.5 (3m8s) |
| 3 | 184.6 (3m4s) |
| **median** | **188.5** |

raw: `raw/after-cargo-build-cold.txt`

### Verdict E2

| Метрика | Baseline | After | Δ |
|---|---|---|---|
| cold-build median | 196.2s | 188.5s | −7.7s (−3.9%) — в пределах шума (variance baseline 3:30→2:42 = ±25%) |
| unused deps в Cargo.toml | 6 | 0 | −6 deps |
| строк в Cargo.toml файлах | — | — | −8 lines |
| `cargo check --workspace` post | ok | ok | 314/314 tests still pass |

Quantitative выигрыш по cold-build около-нулевой — orphan deps в Cargo workspace **не компилируются** если не используются (Rust dependency resolver видит что нет use-цепочки и пропускает). Удаление из `Cargo.toml` — hygiene, не perf.

Что реально удалили — `tokio-test` (dev-dep kepler-backend, никогда не использовали), `axum`/`serde`/`uuid` из ark-relay-server (legacy после refactor'ов), `uuid` из ark-core, `chrono` из kepler-watcher.

**Решение:** оставляем cargo-shear как часть workflow. Добавляю в `lefthook.yml::pre-push` (через `cargo shear` без --fix — только проверка). Гипотеза «perf gain 20-50%» опровергнута, но non-perf бенефит (clean deps, меньший Cargo.lock, чёткие границы) оправдывает adopt.

---

## E3 — Pinia Colada install + миграция одной query

**Гипотеза:** (1) LoC `store/eden.ts` уменьшится после миграции `listEntries`. (2) Eden bundle size вырастет на ~5-10KB gzip (стоимость либы). (3) E2e suite time не изменится.

**Метрика:** (1) `wc -l extensions/eden/src/store/eden.ts`. (2) `ls -la extensions/eden/dist/assets/index-*.js` (raw + gzip). (3) `bunx playwright test tests/e2e/eden.spec.ts` wall-clock.

**Замеры:** 5 × до, 5 × после.

**Scope для E3 этого pass'а:** только **install + plugin wire** (`createApp.use(PiniaColada)`). Без миграции queries/mutations. Цель — убедиться что install drop-in, bundle size осталось в пределах разумного, и подготовить почву для Phase 14 (полная миграция).

### Baseline (до install)

| Метрика | Значение |
|---|---|
| `wc -l extensions/eden/src/store/eden.ts` | 686 |
| Eden bundle `index.js` raw | 358,873 bytes (351 KB) |
| Eden bundle `index.js` gzip | 113,089 bytes (110 KB) |
| `tests/e2e/eden.spec.ts` wall-clock (median последних 3 runs) | **89s (9/9 tests passing)** |

E2e замеры runs 1-2 (5m20s, 4m15s) выпадают — гонялись параллельно с cold-build x3 в фоне, CPU contention. Runs 3-5 (1m30/1m29/1m27) — после освобождения CPU, эти и берём для baseline'а будущей Phase 14 миграции.

raw: `raw/baseline-eden-e2e.txt`

### After (install + `app.use(PiniaColada)`, без миграции queries)

| Метрика | Значение | Δ |
|---|---|---|
| `wc -l extensions/eden/src/store/eden.ts` | 686 | 0 (миграция не делалась) |
| Eden bundle `index.js` raw | 363,421 bytes (355 KB) | +4,548 bytes (+1.3%) |
| Eden bundle `index.js` gzip | 114,819 bytes (112 KB) | **+1,730 bytes (+1.5%)** |

### Verdict E3

Install cost — **+1.7 KB gzipped**, лучше прогноза (5-10KB). Bundle вырос на 1.5%. Drop-in, нет behavior change, typecheck зелёный, `build:extensions` зелёный.

Полная migration store'а на queries/mutations — отдельный proof loop Phase 14. Baseline для миграции зафиксирован здесь (686 LoC `eden.ts`).

---

## E4 — Vitest Browser Mode (component-level testing)

**Не perf-эксперимент — capability add.** Vitest browser + `vitest-browser-vue` + `@vitest/browser-playwright` — новый testing layer между `bun:test` (pure JS) и Playwright e2e (full Electron).

### Setup

- Установлено: `vitest@4.1.6`, `@vitest/browser@4.1.6`, `@vitest/browser-playwright@4.1.6`, `vitest-browser-vue@2.1.0`, `@vitejs/plugin-vue@6.0.7` (peer).
- `extensions/eden/vitest.config.ts` — Vapor interop enabled, alias на `@kosmos/*`, playwright provider, headless Chromium.
- `extensions/eden/tests/components/CharCounter.spec.ts` — пилотный component test для char counter (5 кейсов: empty, 1/2/5 chars, emoji).
- `extensions/eden/package.json::scripts`: `test:unit` (bun), `test:vue` (vitest browser), `test` = оба.

### Verify

```
bun run --cwd extensions/eden test:vue
✓ tests/components/CharCounter.spec.ts (5 tests) 45ms
Test Files  1 passed (1)
     Tests  5 passed (5)
  Duration  8.61s
```

### Verdict E4

Capability add: можно теперь писать Vue component тесты на реальном Chromium. Это страховка от Vapor edge-cases (3.6 beta) — jsdom их не воспроизводит.

В будущем переписать regression-тесты Editor.vue save flow с UI-driven Playwright (тяжёлые, slow) на component-level Vitest browser (~50× быстрее на проверку).

---

## Summary

| E# | Что | Result | Adopt? |
|---|---|---|---|
| E1 | cargo-nextest | −0.3s wall (3.5%) — в пределах шума; gain в UX (progress, retries, JUnit) | ✅ да |
| E2 | cargo-shear cleanup | 6 orphan deps удалено, cold-build не изменился (hygiene > perf) | ✅ да |
| E3 | Pinia Colada install | +1.7 KB gzip, drop-in, baseline зафиксирован для Phase 14 миграции | ✅ да |
| E4 | Vitest browser mode | Новый testing layer, 5/5 sample test passes | ✅ да |
