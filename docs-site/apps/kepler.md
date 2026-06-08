# Kosmos Desktop — Electron host и global launcher

::: tip Источник правды
`platform/desktop/`, `platform/runtime/`
:::

**Kosmos Desktop** — Electron-приложение, которое выступает host'ом для всей Kosmos ecosystem: global launcher по `Alt+Space`, единый runtime для апок, command bus для динамических действий и extension loader для Vue-приложений. В коде этот слой всё ещё называется `kepler-shell` / `kepler:*` для совместимости, но installed app, ярлыки и process name с 2026-05-26 — **Kosmos**.

::: info Brand swap
До 2026-05-14 «Kepler» был именем экосистемы, а launcher назывался иначе. После pivot'а имена swap'нуты, а 2026-05-26 user-facing launcher тоже переехал под один бренд: **Kosmos**. `Kepler` остаётся internal namespace'ом до отдельного cleanup.
:::

## Архитектура

```text
┌────────────────────────────────────────────────────────────┐
│  Kosmos.exe (Electron host, single instance)               │
│   ├─ Kosmos Runtime.exe (Rust child, WS server)            │
│   │    ├─ ARK runtime (объекты, FTS5, sync)                │
│   │    └─ command bus (register/invoke/events)             │
│   ├─ LauncherView (frameless 720×460, Mica/Acrylic)        │
│   ├─ SettingsView (отдельное окно)                         │
│   ├─ Extension host (Phase 4) → BrowserWindow per ext      │
│   ├─ Dashboard / Clipboard / Focus shell command surfaces    │
│   ├─ Tray icon + globalShortcut Ctrl+Shift+K               │
│   └─ IPC к extensions / Electron apps через preload        │
└────────────────────────┬───────────────────────────────────┘
                         │ ws://127.0.0.1:<port>
       ┌──────────┬──────┴────────┬──────────┬─────────────┐
   ┌───┴───┐ ┌────┴────┐    ┌─────┴────┐ ┌───┴──────┐ ┌───┴─────┐
   │ Focus │ │ Delphi  │    │   Eden   │ │Arrancador│ │Dashboard│
   │Session│ │         │    │          │ │          │ │         │
   └───────┘ └─────────┘    └──────────┘ └──────────┘ └─────────┘
```

Каждая extension-апка коннектится к `kepler-backend` через WebSocket (`@kosmos/ark` kepler mode), регистрирует свои команды через [Command bus](../concepts/command-bus.md) и слушает события `command_invoked`. Shell-owned views (Dashboard, Clipboard History, Focus Session) открываются внутренними командами внутри Shell surface. Когда юзер открывает Kosmos launcher и выбирает команду — runtime роутит её к нужной апке или shell surface.

## Стек

| Слой          | Технология                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------- |
| Shell         | Electron 41 (frameless, Mica/Acrylic, transparent)                                          |
| Renderer      | Vue 3.6 + TypeScript + Vite 8 (electron-vite)                                               |
| Bundler       | Vite environments (renderer / main / preload через `vite.config.mjs` в `platform/desktop/`) |
| Backend       | `Kosmos Runtime.exe` packaged name (`platform/runtime`, dev bin `kepler-backend.exe`)       |
| ARK SDK       | `@kosmos/ark` (kepler mode, hello-handshake, command bus client)                            |
| UI            | `@kosmos/visuals` (DesktopChrome, токены, компоненты)                                       |
| Tray / hotkey | Electron `Tray` + `globalShortcut`                                                          |

## Структура

```
platform/desktop/                     # npm package "kepler-shell"
├─ electron/
│  ├─ main.ts              # backend spawn, BrowserWindow, tray, globalShortcut, IPC
│  ├─ preload.ts           # window.kepler API (search / invoke / commands)
│  ├─ extension-preload.ts # preload для extension windows
│  ├─ commands.ts          # builtin static команды (dashboard/clipboard/focus/settings/check-updates)
│  ├─ settings-window.ts   # отдельное окно настроек + IPC handlers
│  ├─ extension-host.ts    # загрузчик Vue extensions
│  ├─ extension-installer.ts / extension-marketplace.ts # установка + catalog lookup
│  ├─ instance.ts          # slot-based isolation (prod / dev / test)
│  ├─ install-extension-window.ts # окно установки .kext / extension package
│  ├─ focus-session.ts / focus-widget.ts / focus-block.ts / focus-service.ts / pomodoro-notifier.ts
│  │                       # focus mode subsystem (см. [Focus mode](../concepts/focus-mode.md))
├─ shared/
│  └─ ipc-types.ts         # KeplerApi (preload contract), CommandRecord, SearchResult
├─ src/
│  ├─ App.vue              # routing (LauncherView / SettingsView по hash)
│  ├─ main.ts              # createApp + Inter Variable
│  ├─ styles.css           # --kepler-accent + локальные токены
│  └─ views/
│     ├─ LauncherView.vue  # секции «Недавние»/«Все» + fuzzy filter + command visibility
│     └─ SettingsView.vue  # sidebar навигация + поиск; страницы: Общие / О приложении /
│                          #   Дебаг / Заметки / Задачи / Фокус-таймер / Игры / Фокус /
│                          #   Расширения / Поиск файлов
├─ src/components/
│  └─ BuiltInIcon.vue      # generic gradient icon (Lucide-based) для builtin команд
├─ scripts/
│  ├─ dev-extensions.mjs   # Vite dev серверы для всех extensions (HMR)
│  ├─ install-extension.mjs   # CLI: положить extension override в %APPDATA%
│  └─ uninstall-extension.mjs
├─ vite.config.mjs            # renderer / main / preload environments
├─ vite.extensions.config.mjs # билд для extensions/
└─ build/                     # иконки + afterPack hook

extensions/                   # ← top-level рядом с platform/desktop/
├─ akasha/  ├─ delphi/  ├─ eden/  ├─ arrancador/
```

## Окно launcher'а

- 720×460 fixed, не resizable, frameless, transparent.
- `backgroundMaterial: 'mica'` (Win11) / acrylic fallback на старых билдах.
- Центрируется на active display.
- `nativeTheme.themeSource = 'dark'` — принудительно тёмная тема, независимо от системы.
- `globalShortcut.register('Ctrl+Shift+K')` (`Cmd+Shift+K` на macOS) — toggle show/hide.
- При потере фокуса — окно **всегда** скрывается (focus-trap pattern, как Spotlight). В dev единственное исключение — focus ушёл в DevTools открытого launcher'а (иначе невозможно дебажить renderer); в production hide-on-blur срабатывает безусловно.
- Tray icon с меню **«Открыть» / «Настройки» / «Выход»**. «Настройки» открывает [отдельное окно настроек](#окно-настроек) через тот же IPC `kepler:settings:open`. Реальный quit — только через tray «Выход».

## Command bus

Kepler — точка входа для всех команд экосистемы. Подробно см. [Command bus](../concepts/command-bus.md).

Коротко:

- `kepler-backend` хранит in-memory registry команд (`commands.register/unregister/list/invoke`) и эмитит события `command_invoked` / `commands_changed` через WS.
- Электронные апки при старте делают `ArkClient.commands.register([...])` и подписываются на `command_invoked` события для своих id'шников.
- `LauncherView` слушает `commands_changed`, держит актуальный список и при выборе вызывает `commands.invoke(id)`.
- Статические команды живут в `electron/commands.ts` и матчатся локально без backend roundtrip — см. ниже.

### Static commands (registry в `platform/desktop/electron/commands.ts`)

Каждая запись — `InternalCommand` с полями `id`, `title`, `subtitle`, `category` (`'open' | 'action'`), `kind` (`'app' | 'command'`), `appName?` и `icon?: () => string | undefined`.

В `platform/desktop/electron/commands.ts` живут kepler-internal команды:

| id                         | kind                            | title                    | Что делает                                                                                           |
| -------------------------- | ------------------------------- | ------------------------ | ---------------------------------------------------------------------------------------------------- |
| `dashboard:open`           | `command` (`appName: "Kepler"`) | Открыть таблицу данных   | `openDashboardWindow()`                                                                              |
| `kepler:clipboard-history` | `command` (`appName: "Kepler"`) | Открыть буфер обмена     | `openClipboardHistoryShell()`                                                                        |
| `kepler:focus-session`     | `command` (`appName: "Kosmos"`) | Начать фокус             | `openFocusSessionShell()`                                                                            |
| `kepler:focus-toggle`      | `command` (`appName: "Kosmos"`) | Переключить фокус        | open start form if idle, complete current session otherwise                                          |
| `kepler:focus-pause`       | `command` (`appName: "Kosmos"`) | Поставить фокус на паузу | `pomodoro.pause` + shell side effects                                                                |
| `kepler:focus-resume`      | `command` (`appName: "Kosmos"`) | Продолжить фокус         | `pomodoro.resume` + shell side effects                                                               |
| `kepler:focus-skip`        | `command` (`appName: "Kosmos"`) | Пропустить фазу фокуса   | `pomodoro.skip`                                                                                      |
| `kepler:focus-complete`    | `command` (`appName: "Kosmos"`) | Завершить фокус          | `pomodoro.stop` + shell side effects                                                                 |
| `settings:open`            | `command` (`appName: "Kepler"`) | Открыть настройки        | `openSettings()`                                                                                     |
| `kepler:check-updates`     | `command` (`appName: "Kepler"`) | Проверить обновления     | `autoupdater.check()` (без открытия Settings); результат — через update banner в launcher и Settings |

Все остальные open-команды (`eden:open`, `delphi:open`, `arrancador:open`, `eden:note:create`, `eden:note:open-today`, `delphi:inbox` и т.д.) **объявляются в `extensions/<id>/manifest.json::commands[]`** и резолвятся `loadDeclaredCommands` из `extension-host.ts`. Источник правды для перечня открывающих команд каждого active extension'а — соответствующий `manifest.json` (см. поле `commands` в [Eden](./eden.md), [Delphi](./delphi.md)).

### Dynamic (action) commands

Регистрируются running extension'ами через [Command bus](../concepts/command-bus.md) (`category: 'action'`). **С 2026-05-19** LauncherView показывает их наравне с open-командами — раньше был фильтр `category !== "action"`, который скрывал dynamic-ручки. Теперь любая зарегистрированная action-команда видна в палитре пока соответствующее приложение запущено и держит WS-connection к kepler-backend.

## Extension host (Phase 4 ✅ + Phase 6.0 ✅)

`platform/desktop/electron/extension-host.ts` — production loader. Активные продуктовые extension-апки рендерятся как Vue extensions внутри Kepler без отдельных Electron-процессов. Dashboard, Clipboard History и Focus Session — встроенные shell views.

- Active extensions лежат в `extensions/<id>/` (top-level, рядом с `platform/desktop/`): **Eden, Delphi, Arrancador, Akasha**.
- Каждое — `manifest.json` + Vue bundle + опциональный preload.
- Host открывает extension в отдельном `BrowserWindow` с reuse через `Map<id, BrowserWindow>`.
- **Dashboard** — встроенный shell view (`platform/desktop/src/views/Dashboard*.vue`), не extension. Открывается через `openDashboardWindow()` из `commands.ts`.
- **Focus Session** — shell-owned command set (`kepler:focus-session`, `kepler:focus-toggle`, `kepler:focus-pause`, `kepler:focus-resume`, `kepler:focus-skip`, `kepler:focus-complete`), не extension. Start/Edit surface открывается внутри текущего Shell через `openFocusSessionShell()`; остальные команды выполняют main-process intents без отдельной формы.
- Eden мигрирован в extension в Phase 6.0 (2026-05-17), standalone `apps/eden/ts/` удалён в Phase 6.0.A.

### Иконки в launcher

Open-command extension'а показывает PNG иконку соответствующего extension'а. `platform/desktop/electron/extension-host.ts → extensionIconDataUri(id)` читает `extensions/<id>/icon.png`, кодирует в data-uri и кэширует **по mtime файла**: при изменении иконки на диске cache автоматически инвалидируется (hot-swap без перезапуска Kepler).

Builtin Kepler-команды без extension PNG (`settings:open`, `dashboard:open`, `kepler:check-updates`) рендерятся через `BuiltInIcon.vue` — gradient-плашка с Lucide-иконкой. Реестр `BUILTIN_ICONS` живёт в `LauncherView.vue` и мапит command id → `{icon, from, to}`: сейчас `settings:open` — серый (`Settings`), `dashboard:open` — teal (`Database`). Команда без записи в реестре получает дефолт компонента — голубой→синий + `HelpCircle`.

`BuiltInIcon` принимает пропсы `icon` (Lucide component), `from` / `to` (gradient stops, OKLCH), `size`, `strokeWidth`. Используется только в launcher; для extension PNG по-прежнему `<img src="data:..." />`.

### Секции и история выбора в launcher

LauncherView показывает две секции, когда строка поиска пустая:

- **Недавние** — до 5 последних invoke'нутых команд в LRU-порядке. Хранится в `localStorage` под ключом `kepler.launcher.recents` (массив id-строк). Запись делается на каждом `invokeSelected()` через `recordRecent(id)`.
- **Все** — полный список команд в порядке регистрации в `COMMANDS` + dynamic поверх (после фильтра `category !== "action"`).

При непустой строке секции скрываются — показывается единый fuzzy-отсортированный список.

`CommandRecord` (`platform/desktop/shared/ipc-types.ts`) расширен полями `kind: 'app' | 'command'` и `appName?`. UI рендерит правую часть row'а так:

- `kind === 'app'` → лейбл «Приложение».
- `kind === 'command'` → приглушённое имя приложения справа от title + лейбл «Команда».

### Crash safety

`BrowserWindow.on("closed", …)` обращается к `win.webContents.id` **после** destroy и крашит процесс. Фикс: `wcId` захватывается **до** регистрации listener'ов (`const wcId = win.webContents.id; win.on("closed", () => webContentsToExtensionId.delete(wcId))`). Без этого Kepler падал при закрытии extension-окна.

RAM-эффект: −124 MB Working Set / −209 MB Private Bytes / −4 процесса (см. [RAM benchmarks](../concepts/ram-benchmarks.md)).

Developer mode с Vite HMR per extension — [Extension dev mode](../concepts/extension-dev-mode.md). Полная архитектура — [Extension host](../concepts/extension-host.md).

## Окно настроек

В Settings:

- **Autostart** toggle — пишет / удаляет ключ `Kosmos` в `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` через `app.setLoginItemSettings`; при миграции читает legacy `Kepler` entry как enabled и чистит его best-effort при следующей записи.
- **Backend status** — состояние `Kosmos Runtime.exe` / dev `kepler-backend.exe` child process'а (running / not running) + текущий порт WS.
- **Версия** — `app.getVersion()`.
- **Developer Mode** toggle (Phase 5) — persist'ится в `%APPDATA%\Kosmos\kepler-shell-settings.json`. Когда включён, extension-host резолвит `loadURL('http://localhost:<devPort>/')` вместо `loadFile(dist/...)` для extension'ов, у которых в `manifest.json` указан `devPort`. F12 в любом extension window открывает DevTools.
- **Update banner** (sticky 32px вверху Settings окна) — Raycast-style индикатор состояния autoUpdater'а. Состояния `available` / `downloading` (progress bar) / `downloaded` (click-to-install) / `error`. Иконки `ArrowUpCircle` / `Loader2` из `@lucide/vue`. Подписан на `window.kepler.settings.update.onStateChanged(...)`. См. [Distribution → autoUpdater](../concepts/distribution.md#kepler-launcher-autoupdater).
- **Кнопка «Проверить обновления»** (General tab) — вызывает `window.kepler.settings.update.check()`. Тот же эффект, что launcher-команда `kepler:check-updates`.
- **Расширения** (таб) — плоский список установленных. Для каждого extension'а:
  - lookup в каталоге: если `catalog.version > installed.version` → кнопка **«Обновить»**;
  - кнопка **«Откатить»** (если есть backup в `extensions-backups/<id>/`);
  - кнопка **«Удалить»**.
  - Сверху таба — кнопка **«Проверить обновления»** (вызывает `loadCatalog(force=true)`).
  - Отдельной секции «Каталог» / «Прочие установленные» больше нет: каталог сейчас используется только для lookup версий, не как картинная витрина.

## Production packaging (Phase 8)

`bun run build` собирает финальный **NSIS one-click** установщик через electron-builder. Конфиг — в `platform/desktop/package.json → build`.

Pipeline:

1. `build:backend` — `cargo build --release --bin kepler-backend`.
2. `build:js` — tsc + vite build (main + preload + renderer).
3. `electron-builder --win nsis` — `release/Kosmos Setup X.Y.Z.exe`.

`extraResources` (копируются рядом с упакованным `Kosmos.exe`):

- `Kosmos Runtime.exe` — packaged `kepler-backend.exe`.
- `Kosmos Data Engine.exe` — packaged `ark-core-rpc.exe`; это всё ещё отдельный child process. One-process runtime — отдельная будущая задача.
- `Kosmos Helper.exe` — packaged `kepler-focus-helper.exe`.
- `Kosmos System Service.exe` — packaged `kepler-focus-svc.exe`.
- `extensions/` — bundle'ы Vue extensions (только `manifest.json`, `icon.png`, `index.html`, `dist/`; исключаются `src/`, `node_modules/`, `package.json`, vite configs).
- `icon.png` — для tray и `BrowserWindow.icon`.

`afterPack` (`build/afterPack.cjs`) embed'ит иконку в `Kosmos.exe` через `rcedit` + `png-to-ico` (workaround под отключённый встроенный rcedit electron-builder из-за `win.signAndEditExecutable: false`).

NSIS-настройки: `oneClick: true`, `perMachine: false` (install в `%LocalAppData%\Programs\Kosmos` без UAC), `runAfterFinish: true`, desktop + Start Menu shortcut `Kosmos`, `deleteAppDataOnUninstall: false` (не теряем данные при апдейте). Custom install удаляет legacy `Kepler.lnk`.

Что ещё в Phase 8 (⏳): `electron-updater` для auto-update. Legacy Rust gpui launcher уже удалён в Phase A (директория apps/kepler/ больше не существует).

## Extension installer (Phase 10 MVP)

Dashboard в этот список **не входит** — после 2026-05-14 он встроенный shell view (см. [Dashboard](/apps/dashboard)), не extension.

```powershell
# install: <path-to-extension-dir> должен содержать manifest.json, dist/, icon.png
bun run --cwd platform/desktop ext:install ./products/delphi
# uninstall
bun run --cwd platform/desktop ext:uninstall delphi
```

Подробно (atomic копирование, layout, что НЕ входит в MVP — auto-update, `.kext` формат, UI manager) — [Extension installer](../concepts/extension-installer.md).

## Запуск (dev)

```powershell
cd platform/desktop
bun run build:backend:dev   # cargo build (debug) platform/runtime
bun run dev                 # build:backend:dev + extensions + vite + Electron
```

`Ctrl+Shift+K` глобально откроет launcher. Tray-иконка появится в трее.

## Команды

| Команда                                             | Что                                                                                                                                          |
| --------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `bun run --cwd platform/desktop dev`                | dev режим                                                                                                                                    |
| `bun run --cwd platform/desktop build:js`           | tsc + vite build (без NSIS)                                                                                                                  |
| `bun run --cwd platform/desktop build`              | release backend + js + NSIS installer                                                                                                        |
| `bun run --cwd platform/desktop typecheck`          | tsc --noEmit                                                                                                                                 |
| `bun run --cwd platform/desktop package:dir`        | unpacked Electron сборка                                                                                                                     |
| `bun run --cwd platform/desktop test:e2e`           | Playwright e2e                                                                                                                               |
| `bun run --cwd platform/desktop ext:install <path>` | поставить extension в `%APPDATA%\Kosmos\extensions\<id>\` (override bundled). См. [Extension installer](../concepts/extension-installer.md). |
| `bun run --cwd platform/desktop ext:uninstall <id>` | удалить user-installed extension; bundled (если есть) поднимется автоматически.                                                              |

Артефакты `build` — `platform/desktop/release/Kosmos Setup X.Y.Z.exe` (NSIS one-click).

## История legacy Rust-launcher'а

Старый Rust-launcher (gpui + tao + global-hotkey) лежал в apps/kepler и удалён в Phase A (после brand swap'а 2026-05-14). Вся работа идёт через `platform/desktop/`.

## См. также

- [Roadmap](./kepler-roadmap.md) — фазы миграции и план Phase 4-6.
- [Command bus](../concepts/command-bus.md) — протокол dynamic commands.
- [@kosmos/ark](../packages/ark.md) — TS SDK с `commands` namespace.
- [Architecture](../concepts/architecture.md) — общая картина Kosmos.
