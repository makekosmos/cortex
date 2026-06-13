# Тесты / e2e / headless запреты

::: tip Узкий файл
Читайте только когда задача касается этой области. Полный legacy reference: `docs-site/agents/forbidden.md`.
:::

## Тесты

- ❌ Запускать e2e без `KOSMOS_HEADLESS=1`. Окна Kosmos / extension'ов не должны лезть на экран и воровать focus у пользователя. `launchKepler` helper выставляет этот env автоматически — не override'ить в `opts.env`.
- ❌ Создавать BrowserWindow в `platform/desktop/electron/` без проверки `process.env.KOSMOS_HEADLESS === "1"`. Любое новое окно должно респектать headless mode (`show: !headless`, `skipTaskbar: headless`).
- ❌ Звать `.show()` / `.showInactive()` / `.focus()` / `.setAlwaysOnTop(true)` на BrowserWindow в `platform/desktop/electron/` без headless guard'а. `showLauncher`, `focusExistingExtensionWindow`, `showWidget`, `openSettings`, `openDashboardWindow`, `openInstallExtensionWindow` — все должны раннее return'ить в headless/test mode (либо пропускать визуальные операции, оставляя state / IPC). Видимые окна во время e2e — это **баг**, а не «фича тестов». Пользователь не должен видеть мигающего UI от прогона.
- ❌ Полагаться на `getByRole("button", { name: /<text>/ })` strict-mode, если на странице потенциально несколько подходящих кнопок (Vue transitions, multi-pane layouts). Scope'ить к специфичному CSS классу. См. [Testing → ловушки](/agents/testing#vue-transition).
- ❌ Lazy-регистрировать object_type extension'ом при первом write если extension объявлен в `manifest.tests.smoke`. Eager register на boot в shim'е — иначе universal contract spec падает с FK constraint.
- ❌ Дефолт пути к user ARK DB (`%APPDATA%\Kosmos\ark.db`) в тестах.
- ❌ Захардкоженный путь к real user dir (типа `C:\Users\me\AppData\...`).
- ❌ Запуск миграции/backfill против реальной ARK DB «чтобы проверить».
- ❌ Запуск Playwright против user vault Eden.
- ❌ Указывать тестам `KOSMOS_DATA_DIR` равным `%APPDATA%\Kosmos\` (real user data). Backend поддерживает `KOSMOS_DATA_DIR` override именно чтобы тесты могли подсунуть свой dir под `tests/.e2e/<spec>/`. Helper `tests/e2e/helpers/launch.ts` явно отказывается принимать путь внутри `%APPDATA%`.
- ❌ Запускать Playwright без `KOSMOS_DATA_DIR` override — тогда backend упадёт в user data dir.
- ❌ Хардкодить `path.join(appData, "Kosmos", ...)` или `"Kepler"` userData в shell / extension main process. Используй `resolveInstance()` / `keplerDataDir()` из `platform/desktop/electron/instance.ts` — single source of truth для slot-based изоляции (`prod` / `dev` / `dev-<x>` / `test-<x>`). См. [Instance slots](/concepts/instances). Иначе dev/test/multi-dev изоляция тихо ломается.
- ❌ Звать `app.requestSingleInstanceLock()` ДО `applyInstanceToApp(resolveInstance())` в `platform/desktop/electron/main.ts`. Lock scope'ится по `app.getPath('userData')`; если он ещё «Kepler» (default) — prod и dev делят один lock и второй инстанс молча выходит. Порядок в module top-level main.ts критичен.
- ❌ `setLoginItemSettings({ path: process.execPath, ... })` без проверки `resolveInstance().autorunEnabled`. Из dev процесса `process.execPath` это `electron.exe` из `node_modules/` — нечего прописывать в HKCU Run.
- ❌ Включать `bun run --cwd platform/desktop dev` в production install path или launcher для конечного юзера. Dev mode пишет в `Kosmos-dev/`, а production install — в `Kosmos/`. Путать их → разные данные у разработчика и установленного приложения.
