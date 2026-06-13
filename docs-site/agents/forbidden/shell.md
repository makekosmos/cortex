# Shell / command bus / brand запреты

::: tip Узкий файл
Читайте только когда задача касается этой области. Полный legacy reference: `docs-site/agents/forbidden.md`.
:::

### Kepler Shell (launcher)

- ❌ Возврат к ARK FTS5 search внутри лаунчера вместо command bus (был pivot — отброшен).
- ❌ Per-frame window resize animation: Win32 не успевает, окно дёргается. Размер окна — fixed 720×460.
- ❌ Hardcoded extension команды в `platform/desktop/electron/commands.ts`. `COMMANDS[]` содержит только shell-owned/internal commands (`settings:open` / `dashboard:open` / `kepler:clipboard-history` / Focus command set `kepler:focus-*` / `kepler:check-updates`). Extension-команды (например `eden:open`, `delphi:inbox`) объявляются в `extensions/<id>/manifest.json` `commands[]` — manifest = source of truth. `loadDeclaredCommands` в `extension-host.ts` собирает их. Видны в launcher всегда (пока extension установлен), uninstall их убирает. См. [Command bus → три слоя](/concepts/command-bus#архитектура-три-слоя-команд-v1-v2-2026-05-19).
- ❌ Использование `win.webContents.id` внутри `closed` event handler. После `closed` webContents уже destroyed — capture id в локальную `const wcId` **до** `win.on("closed", ...)`. См. [Extension host → Crash safety](/concepts/extension-host#crash-safety).
- ❌ Удаление `electron-api-shim.ts` в Delphi extension. Это compat-слой эмулирующий `window.electronAPI` поверх kepler ark bridge — без него сломаются ~30 call sites Delphi CRUD без переписывания. Миграция UI на нативный API — отдельная Phase 9.
- ❌ Загрузка extension renderer с `file://path/to/dist` когда хочешь HMR. В dev mode (Settings → Developer Mode toggle, **не** `KEPLER_DEV=1`) используй `loadURL('http://localhost:<devPort>/')` с поднятым Vite dev server'ом. См. [Extension dev mode](/concepts/extension-dev-mode).
- ❌ Загружать extension из `http://localhost:<devPort>/` без TCP probe порта. Архитектура с 2026-05-19 — `resolveExtensionSource()` в `extension-host.ts` всегда вызывает `probeExtensionDevServer(port)` (timeout 500ms) перед тем как взять Vite-путь; если порт мёртв — graceful fallback на `dist/`. Старая логика (грузить с localhost когда `developerMode: true` в settings) приводила к пустым окнам если dev server упал/не поднят, и persisted setting утекал в installed Kepler.
- ❌ Делать `openExtension(id)` синхронным или без in-flight Map. Probe — async, поэтому два rapid invoke на один id без дедупа создадут два BrowserWindow'а. См. `openInflight` Map в `extension-host.ts`.
- ❌ Дублирование install-flow логики (backup / atomic rename / semver-проверка). Источник правды — `platform/desktop/electron/extension-installer.ts`. CLI скрипт `platform/desktop/scripts/install-extension.mjs` копирует semver matcher inline (~40 строк) только потому, что mjs скрипт не имеет доступа к dist-electron bundle; не размножай это в третьем месте — дёргай IPC `kepler:extension:install:do` или сам runtime API.
- ❌ Ослабление `keplerApiVersion` compat check в `extension-host.ts → checkApiCompat()`. Несовместимый extension **не** должен получать live preload bridge — иначе ломается инвариант API contract'а. Если правишь — bump `KEPLER_API_VERSION` в `platform/desktop/electron/kepler-api.ts` соответственно (patch/minor/major по семантике).
- ❌ Path traversal в `.kext` extract'е. `extension-installer.ts → safeEntryName` отвергает `..`, абсолютные пути, drive letter'ы. Не упрощай эту проверку — `.kext` может приехать из untrusted источника.
- ❌ Выход из `initArkClient()` без вызова `arkClientReadyReject?.()` когда `state.kind !== "connected"`. До 2026-05-18 функция тихо `return`'ила и все pending `awaitArkReady()` висели 15 секунд до generic timeout. Каждый failure path в `initArkClient` должен либо reject'нуть resolver, либо успешно его resolve'нуть.
- ❌ Перезапуск `kepler-backend` (через `kepler:backend:restart`) или его exit без вызова cleanup'а `arkClient` (stop + null + сброс `arkClientReady`). Без этого WS-соединение указывает на мёртвый порт и `invokeOperation` зависает на reconnect-логике клиента, не возвращая ошибки. См. `resetArkClient` helper в `platform/desktop/electron/main.ts`.
- ❌ `setTimeout` в `Promise.race([..., new Promise((_, rej) => setTimeout(rej, ms))])` без `clearTimeout` на successful resolve. Таймер продолжает держать event loop до полного TTL даже после того как race выиграла другая ветка.

### Command bus

- ❌ Nested wire format событий `{kind: "event", type: "...", payload: {...}}`. Только flat: `{event: "...", ...fields}` — это согласовано с peer/sync events.
- ❌ Регистрация commands вне `kepler-mode`. Self-managed / standalone-запуск приложения **не** должен падать из-за отсутствия commands API — оборачивай в `try/catch`.
- ❌ Прямой WS-доступ к backend из renderer'а приложений в обход `@kosmos/ark` SDK.

### Brand consistency

- ❌ Возврат наружного product name **Kepler** для установленного desktop app. С 2026-05-26 пользовательский продукт — **Kosmos** (`Kosmos.exe`, ярлыки `Kosmos`, `%APPDATA%/Kosmos App` для Electron userData). `Kepler` допускается только как legacy/internal namespace (`kepler:*` IPC, `kepler-backend`, `kepler.lock.json`) до отдельного cleanup.
- ❌ Массовый rename внутренних namespaces (`window.kepler`, `kepler:*`, `platform/runtime`, `kepler.lock.json`) без отдельного proof loop. Эти имена — совместимость протокола и тестов, а не пользовательский бренд.
- ❌ Возврат npm scope `@kepler/*` для shared пакетов. Runtime и UI общие для всей экосистемы используют **`@kosmos/*`**: `@kosmos/ark`, `@kosmos/visuals`. Между Phase B4 (2026-05-14) и 2026-05-18 они некоторое время жили под `@kepler/*` — это была ошибка naming'а (ARK и visuals shared across all apps, не launcher-specific). `@kepler/*` зарезервирован для launcher-specific пакетов, если такие появятся.
