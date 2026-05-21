# 2026-05-21 bug-detection-phase1-determinism

## Context

E2e suite в `tests/e2e/` содержит **121 `waitForTimeout(...)`** на 27 spec'ах (топ:
`eden.spec.ts` — 23, `eden-selection-after-click.spec.ts` — 20,
`eden-taskref-arrow-nav.spec.ts` — 13). Это flake-источник: тесты молча проходят
локально и падают на slow CI / cold backend warmup.

Корневая причина — нет deterministic-сигнала от приложения о готовности.
`shell/electron/main.ts` имеет только pull `ipcMain.handle("kepler:backend:status")`,
broadcast'а `kepler:backend:ready` нет. `awaitArkReady()` ждёт promise внутри
main process, но renderer / тесты узнают о готовности либо через polling, либо
через жёсткие `waitForTimeout(2500)`.

Это Phase 1 из большого плана «меньше времени на фикс багов» (см. чат
2026-05-21). Cледующие фазы (CI / ESLint / ts-rs / logging) в отдельных
proof loop'ах.

## Scope

В задаче:

- Broadcast `kepler:backend:ready` всем BrowserWindow когда `initArkClient` успешно
  отрезолвился; broadcast `kepler:backend:disconnected` в `resetArkClient`.
- В `shell/electron/preload.ts` и `shell/electron/extension-preload.ts`:
  expose `kepler.backend.onReady(cb)` (production-safe, без state leak'а).
- Test rig `kepler.__test` exposed **только** при `KOSMOS_TEST_MODE === "1"`:
  - `waitForReady(timeoutMs?): Promise<void>`
  - `getStats(): { arkConnected, commandsRegistered, extensionsLoaded }`
- Новый helper `tests/e2e/helpers/wait.ts` с `waitForBackendReady`,
  `waitForCommandRegistered`.
- `bun add -D electron-playwright-helpers` в root.
- Миграция helpers `eden.ts` / `horologion.ts` / `launch.ts` на новые wait'ы
  где это снимает `waitForTimeout`.
- Mechanical sweep `waitForTimeout` в топ-3 spec'ах (eden.spec, eden-selection,
  eden-taskref-arrow-nav) — там где timeout = ожидание backend readiness /
  command registration. UI-таймауты (animation / debounce) **не** трогаем.

Не в задаче:

- ESLint / clippy / CI (отдельные phase'ы).
- ts-rs (Phase 5).
- Visual regression / component tests (Phase 6).
- Полная замена всех 121 `waitForTimeout`. Только те, что относятся к
  backend/command readiness. Остальные (animation, debounce) — отдельная
  ревизия позже.

## Acceptance Criteria

AC1. `bun run --cwd shell typecheck` — зелёный после правок main/preload.

AC2. В production preload (`KOSMOS_TEST_MODE` unset) `window.kepler.__test`
     === `undefined`. Проверка: запустить `bun run --cwd shell dev`, в DevTools
     `window.kepler.__test` === undefined.

AC3. В test mode (`KOSMOS_TEST_MODE=1`) `window.kepler.__test.waitForReady()`
     резолвится в течение 15 сек на cold start, и `getStats().arkConnected` ===
     true после резолва.

AC4. `bun run test:e2e` — все specs зелёные.

AC5. Wall-clock топ-3 spec'ов (eden, eden-selection-after-click,
     eden-taskref-arrow-nav) после миграции — **меньше или равно** baseline'у.
     Baseline записан до правок в `evidence.md`.

AC6. `bun run ark:guard:writes` — зелёный.

AC7. Все нарушения `KOSMOS_HEADLESS` / `KOSMOS_TEST_MODE` гвардов остаются на
     месте (никаких regressions в `forbidden.md::E2e тесты — всегда невидимые`).

## Verification commands

- `bun run --cwd shell typecheck` — AC1.
- Manual DevTools check — AC2, AC3.
- `bun run test:e2e` — AC4, AC5.
- `bun run ark:guard:writes` — AC6.
- `grep -nE 'show: |showInactive|setAlwaysOnTop' shell/electron/*.ts` —
  визуальная ревизия что guards не сняли (AC7).

## Out of scope decisions

- Замена остальных ~80 `waitForTimeout` (UI debounce / animation) — отдельная
  ревизия после Phase 1.
- Если test rig `__test.getStats` нужен extension'ам для их тестов — добавим
  в `extension-preload.ts` отдельно. Сначала покрываем shell preload.
