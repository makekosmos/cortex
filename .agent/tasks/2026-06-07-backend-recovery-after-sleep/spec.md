# 2026-06-07 backend-recovery-after-sleep

## Context

На macOS после длительного сна ноутбука лаунчер открывается пустым: ни
приложений, ни команд. Диагностика (логи dev-слота 2026-06-07): backend-процесс
(`kepler-backend`) умер во время сна (последняя запись 09:48, дальше тишина — ни
panic, ни shutdown), electron-main выжил, но supervisor бэкенд **не перезапустил**
(нет `respawning` в логе). ArkClient остался отвязан → все `kepler:ark:request`
таймаутят (`ArkClient not ready (timeout)`) → `app_index.list_all` и динамические
команды недоступны → лаунчер пуст. Сам бинарь бэкенда здоров (автономный запуск:
WS up, 221 mac-приложение).

Корень: восстановление зависит только от события `backendProc.on("exit")`.
При `code === 0` supervisor намеренно не respawn'ит (main.ts:382), а во время
suspend событие `exit` могло вообще не доставиться. Реакции на
`powerMonitor` resume нет. Нет и on-demand health-recovery при показе лаунчера.
Renderer не подписан на `backend.onReady`, поэтому даже при реконнекте список
не перезапрашивается до следующего show.

## Scope

В задаче:

- `main.ts`: активный health-probe ArkClient + `recoverBackendIfDead(reason)`
  (reset → kill stale → spawn → init), идемпотентный, не вмешивающийся в штатный
  (re)connect/respawn.
- Триггеры recovery: `powerMonitor.on("resume")` и `showLauncher()` (self-heal
  при открытии лаунчера после сна).
- Гейты против стартовой гонки: `arkInitInFlight` (флаг в `initArkClient`) +
  `bootInitStarted` (выставляется перед первым `initArkClient` в whenReady).
- Renderer `LauncherView.vue`: подписка на `window.kepler.backend.onReady` →
  `refreshCommands()`, чтобы список наполнился сразу после реконнекта.

Не в задаче:

- Изменение supervisor-политики `code === 0 → no respawn` (recovery-слой
  покрывает пробел независимо от надёжности `exit`-события).
- UI-индикатор «бэкенд переподключается» (отдельная задача; пока self-heal тихий).
- Windows-специфика (фикс кросс-платформенный, resume актуален и там).

## Acceptance Criteria

AC1. После убийства живого backend-процесса recovery поднимает новый и ArkClient
реконнектится: проверяется live на маке (kill pid → trigger → новый pid в
lock-файле + `kepler:ark:request` снова отвечает).

AC2. `recoverBackendIfDead` не запускает дублирующий backend во время штатного
boot-handshake (гонка `showLauncher` до `initArkClient`) — гейт `bootInitStarted`
/ `arkInitInFlight`.

AC3. `LauncherView.vue` подписывается на `backend.onReady` и вызывает
`refreshCommands`; отписка в `onUnmounted`.

AC4. typecheck (`tsc --noEmit`) зелёный; vue-tsc не добавляет ошибок в
изменённые файлы.

## Verification commands

- Live mac: `lsof`/`pgrep` kepler-backend, `kill <pid>`, дернуть recovery
  (powerMonitor resume эмулировать сложно → проверяем через showLauncher путь и
  через прямой вызов), убедиться в новом pid + рабочем ark-request — AC1.
- Code review гейтов — AC2.
- `grep backend.onReady platform/desktop/src/views/LauncherView.vue` — AC3.
- `bun run --cwd platform/desktop typecheck` — AC4.

## Out of scope decisions

- Postmortem в `docs-site/agents/postmortems.md` (bug-postmortem skill).
