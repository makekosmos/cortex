# 2026-05-22 bug-detection-phase7-boot-check

## Context

Bug-detection roadmap Phase 7. После Phase 4 (`keplerLog` infrastructure) и
Phase 6 у нас есть structured logging, но shell main process ничего не
проверяет на старте: если invariant'ы slot isolation сломаны (например
кто-то прокинул `KEPLER_INSTANCE=prod` но забыл вызвать
`applyInstanceToApp`), Kepler пишет в чужой userData молча.

Аналогично — IPC handler'ы в `main.ts` (`kepler:objects:listRecent`,
`kepler:search:query`, `kepler:commands:invoke`, etc.) при exception'е молча
ломаются на renderer side: renderer получает rejected promise без stack'а в
shell-логах. Phase 4 `keplerLog` дал инфраструктуру, но handler'ы её не
используют — каждый автор должен помнить про try/catch.

## Scope

В задаче:

- **Boot self-check** в `shell/electron/main.ts::app.whenReady()`. После
  `applyInstanceToApp(KEPLER_INSTANCE)` (выполняется на module top-level)
  и до `spawnBackend()`:
  - `app.getPath('userData') === KEPLER_INSTANCE.userDataDir` — иначе
    `dialog.showErrorBox` + `app.exit(1)`. Защита от ситуации когда
    что-то ниже по импортной цепочке успело вызвать `app.getPath` до
    `applyInstanceToApp`.
  - `existsSync(resolveBackendExe())` — иначе error dialog
    «kepler-backend.exe не найден» + exit. Защита от sloppy installer.
  - `KEPLER_INSTANCE.slot !== "test"` если `!process.env.KOSMOS_TEST_MODE`.
    Sanity — `test-<x>` slot не должен резолвиться в обычном запуске.
  - Все checks логируются через `keplerLog` (success path тоже —
    «boot self-check passed» с meta).
- **`safeHandle` wrapper** в `shell/electron/ipc-safe.ts` (новый файл):
  `safeHandle(channel, handler)` оборачивает `ipcMain.handle`, при throw
  логирует `keplerLog.error("ipc", "<channel> threw", { err, stack })` и
  re-throw'ит (renderer получит rejection как обычно). Это инфраструктура
  для будущего sweep'а всех handler'ов.
- **Mechanical sweep** — заменить ~10 пилотных `ipcMain.handle(...)` в
  `main.ts` на `safeHandle(...)`. Кандидаты — handler'ы которые сейчас
  без try/catch обёртки или с минимальной обработкой:
  `kepler:objects:listRecent`, `kepler:search:query`,
  `kepler:commands:invoke`, `kepler:commands:list`, `kepler:export:list`,
  `kepler:export:run`, `kepler:export:pickDir`, `kepler:ark:request`,
  `kepler:backend:restart`, `kepler:crashes:list`. Остальные ~20+
  handler'ов оставляем — это infrastructure для будущего полного sweep'а.

Не в задаче:

- Sweep всех 30+ IPC handler'ов в `main.ts` / `extension-host.ts` /
  `settings-window.ts` — отдельная фаза.
- UI индикатор boot failure (просто dialog + exit).
- Retry / recovery для boot self-check failure — fail-loud, не
  пытаемся «починить».

## Acceptance Criteria

AC1. `shell/electron/main.ts::app.whenReady()` начинается с вызова
`runBootSelfCheck()` (или эквивалентного inline блока) который проверяет
три invariant'а из Scope. Каждый failure path вызывает
`dialog.showErrorBox` + `app.exit(1)`.

AC2. `shell/electron/ipc-safe.ts` существует, exports `safeHandle`. Signature:
`safeHandle<P[], R>(channel: string, handler: (event, ...args: P) => Promise<R>): void`.

AC3. Минимум 10 IPC handler'ов в `main.ts` сконвертированы на `safeHandle`.

AC4. `bun run --cwd shell typecheck` зелёный.

AC5. `bun run --cwd shell build:js` зелёный.

AC6. `bunx playwright test tests/e2e/eden.spec.ts` зелёный (9/9). Regression
check.

AC7. `bunx oxlint .`, `bunx oxfmt --check .`, `bun run ark:guard:writes` —
зелёные.

## Out of scope decisions

- `safeHandle` re-throw'ит (не swallow'ит) — renderer должен увидеть
  rejection как обычно, иначе UI отображает «successful» при failure.
  Логирование — дополнение, не подмена.
- Boot self-check exit code 1 (не 2 или 3) — single error path, не различаем
  типы failure'ов на уровне exit code.
