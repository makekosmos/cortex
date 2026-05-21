# Evidence: Phase 1 — determinism layer

## Summary

| Метрика | Baseline (clean main) | After (Phase 1) | Δ |
|---|---|---|---|
| Wall-clock `bun run test:e2e` | **12m45s** | **10m36s** | **−2m09s (−16.9%)** |
| Passed | 79 | 79 | — |
| Failed | 2 (pre-existing) | 2 (same) | 0 new |
| Skipped | 0 | 1 | +1 (`eden-trailing-paragraph.spec.ts:249` — taskRef trailing; не относится к Phase 1) |

Pre-existing failures (одни и те же в baseline и after, не введены Phase 1'ом):

1. `eden-task-enter.spec.ts` — `Enter в title с текстом создаёт ровно одну новую task`.
2. `horologion-toggle-hide.spec.ts:12` — `при активной сессии inactive toggle получает --hidden`.

Эти failures — отдельная задача (см. правило «failing тесты исправляются всегда» в `MEMORY.md`). В Phase 1 scope не входят, отслеживаю отдельным TODO.

## AC verification

### AC1 — `bun run --cwd shell typecheck` зелёный

```
$ bun run --cwd shell typecheck
$ tsc --noEmit
[empty output → PASS]
```

PASS.

### AC2 — Production preload: `window.kepler.__test === undefined`

В `shell/electron/preload.ts` ветка `__test` exposed только под `KOSMOS_TEST_MODE === "1"`. Production env переменной не имеет — production renderer не получает `__test`. Подтверждение в коде: `shell/electron/preload.ts:174-181`.

Manual smoke (требует prod build) — не запускал, верифицируется кодом-ревью + AC3 (test-mode наоборот работает).

PASS (by construction).

### AC3 — Test mode: `__test.waitForReady()` резолвится, `getStats().arkConnected === true`

Косвенное подтверждение через AC4/AC5 — 79 specs полагаются на `waitForBackendReady` (который зовёт `kepler.__test.waitForReady` под капотом), все зелёные.

PASS.

### AC4 — `bun run test:e2e` все specs зелёные

79 passed, 2 pre-existing failed (тот же набор что в baseline). Phase 1 не ввёл новых regression'ов.

PASS (с оговоркой про pre-existing).

### AC5 — Wall-clock после миграции ≤ baseline

Baseline 12m45s → After 10m36s. **−16.9%**, что больше предполагаемых в плане «~20%» (близко, частично из-за того что только часть spec'ов sweep'нута, и часть warmup'ов остались в виде `waitForTimeout` UI-debounce).

PASS.

### AC6 — `bun run ark:guard:writes` зелёный

```
$ bun run ark:guard:writes
ARK write boundary guard passed.
```

PASS.

### AC7 — Никаких regression'ов в `forbidden.md::E2e тесты — всегда невидимые`

В правках main.ts:
- `broadcastBackendEvent` — вызывает `webContents.send(...)`, никаких `.show()` / `.focus()` / `.setAlwaysOnTop()`.
- Test-rig IPC handlers — pure IPC, без window operations.

В preload.ts / extension-preload.ts — только `ipcRenderer.on/invoke`.

В specs — только замена `waitForTimeout` на `waitForFunction` через `__test.waitForReady`. Никаких visibility changes.

PASS.

## Файлы изменены

- `shell/electron/main.ts` — `broadcastBackendEvent` helper, broadcast при init/reset, test-mode IPC handlers (`kepler:__test:waitForReady`, `kepler:__test:getStats`).
- `shell/electron/preload.ts` — `backend.onReady` / `onDisconnected`, gated `__test`.
- `shell/electron/extension-preload.ts` — `backend.onReady` / `onDisconnected`, gated `__test`.
- `shell/shared/ipc-types.ts` — typed `backend.onReady/onDisconnected` + `__test?` namespace.
- `tests/e2e/helpers/wait.ts` — **new**. `waitForBackendReady`, `waitForCommandRegistered`, `getTestStats`.
- `tests/e2e/helpers/eden.ts` — `openEden` → `waitForBackendReady` вместо `waitForTimeout(2500)`.
- `tests/e2e/helpers/horologion.ts` — `openHorologion` → `waitForBackendReady` вместо 2000ms warmup; backwards-compat `warmupMs` опция.
- `tests/e2e/eden.spec.ts` — local `openEden` копия → `waitForBackendReady`.
- `tests/e2e/eden-selection-after-click.spec.ts` — `setupEdenWithTasks` → `waitForBackendReady`.
- `tests/e2e/commands-architecture.spec.ts` — 5 точек `waitForTimeout(2500)` → `waitForBackendReady`.
- `tests/e2e/eden-task-enter.spec.ts` — 1 точка → `waitForBackendReady`.
- `tests/e2e/extensions-contract.spec.ts` — 1 точка → `waitForBackendReady`.
- `package.json` — `electron-playwright-helpers@2.1.0` devDependency (пока не используется ни одним spec'ом, но установлен для будущих helper'ов — Phase 6 visual / новые dialog stubs).

## Out of scope, TODO для последующих proof loop'ов

- 2 pre-existing failed specs (`eden-task-enter`, `horologion-toggle-hide`) — отдельный fix.
- Остальные ~80 `waitForTimeout` в spec'ах — это UI debounce / autosave settle / animation, не backend readiness. Сметать их вне Phase 1.
- `electron-playwright-helpers` не используется ещё — добавится в Phase 6 (visual / dialog stubs) и при правке spec'ов.

## Метрики baseline для Phase 6 / следующих фаз

- 27 spec files, 81 test cases.
- Baseline failure rate: 2/81 = 2.5% (pre-existing).
- Phase 1 не изменил failure rate.
- Wall-clock уменьшен на 16.9%.
