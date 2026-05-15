# Kepler — Electron host и global launcher

::: tip Источник правды
`shell/`, `services/kepler-backend/`
:::

**Kepler** — Electron-приложение, которое выступает host'ом для всей Kosmos ecosystem: global launcher по `Ctrl+Shift+K`, единый backend для апок, command bus для динамических действий и (Phase 4+) extension loader для Vue-приложений. Это **сердце десктопа Kosmos**: один процесс держит ARK, маршрутизирует команды и (в перспективе) рендерит сами апки как extensions.

::: info Brand swap
До 2026-05-14 «Kepler» был именем экосистемы, а launcher назывался иначе. После pivot'а имена swap'нуты: **Kosmos** = ecosystem (общий ARK, общая БД, общие пакеты), **Kepler** = host-приложение / launcher.
:::

## Архитектура

```text
┌────────────────────────────────────────────────────────────┐
│  Kepler (Electron host, single instance)                   │
│   ├─ kepler-backend.exe (Rust child, WS server)            │
│   │    ├─ ARK runtime (объекты, FTS5, sync)                │
│   │    └─ command bus (register/invoke/events)             │
│   ├─ LauncherView (frameless 720×460, Mica/Acrylic)        │
│   ├─ SettingsView (отдельное окно)                         │
│   ├─ Extension host (Phase 4) → BrowserWindow per ext      │
│   ├─ Tray icon + globalShortcut Ctrl+Shift+K               │
│   └─ IPC к extensions / Electron apps через preload        │
└────────────────────────┬───────────────────────────────────┘
                         │ ws://127.0.0.1:<port>
       ┌──────────┬──────┴────────┬──────────┬─────────────┐
   ┌───┴───┐ ┌────┴────┐    ┌─────┴────┐ ┌───┴──────┐ ┌───┴─────┐
   │Horolo-│ │ Delphi  │    │   Eden   │ │Arrancador│ │Dashboard│
   │ gion  │ │         │    │          │ │          │ │         │
   └───────┘ └─────────┘    └──────────┘ └──────────┘ └─────────┘
```

Каждая Electron-апка коннектится к `kepler-backend` через WebSocket (`@kepler/ark` kepler mode), регистрирует свои команды через [Command bus](../concepts/command-bus.md) и слушает события `command_invoked`. Когда юзер открывает Kepler launcher и выбирает команду — backend роутит её к нужной апке.

## Стек

| Слой | Технология |
|---|---|
| Shell | Electron 41 (frameless, Mica/Acrylic, transparent) |
| Renderer | Vue 3.6 + TypeScript + Vite 8 (electron-vite) |
| Bundler | Vite environments (renderer / main / preload через `vite.config.mjs` в `shell/`) |
| Backend | `kepler-backend.exe` (Rust, lib + bin из `services/kepler-backend/`) |
| ARK SDK | `@kepler/ark` (kepler mode, hello-handshake, command bus client) |
| UI | `@kepler/visuals` (DesktopChrome, токены, компоненты) |
| Tray / hotkey | Electron `Tray` + `globalShortcut` |

## Структура

```
shell/                     # npm package "kepler-shell"
├─ electron/
│  ├─ main.ts              # backend spawn, BrowserWindow, tray, globalShortcut, IPC
│  ├─ preload.ts           # window.kepler API (search / invoke / commands)
│  ├─ extension-preload.ts # preload для extension windows
│  ├─ commands.ts          # статические команды (open app tiles + builtin Kepler commands)
│  ├─ settings-window.ts   # отдельное окно настроек + IPC handlers
│  └─ extension-host.ts    # загрузчик Vue extensions
├─ shared/
│  └─ ipc-types.ts         # KeplerApi (preload contract), CommandRecord, SearchResult
├─ src/
│  ├─ App.vue              # routing (LauncherView / SettingsView по hash)
│  ├─ main.ts              # createApp + Inter Variable
│  ├─ styles.css           # --kepler-accent + локальные токены
│  └─ views/
│     ├─ LauncherView.vue  # секции «Недавние»/«Все» + fuzzy filter
│     └─ SettingsView.vue  # настройки host'а (hotkey, backend status)
├─ src/components/
│  └─ BuiltInIcon.vue      # generic gradient icon (Lucide-based) для builtin команд
├─ scripts/
│  ├─ dev-extensions.mjs   # Vite dev серверы для всех extensions (HMR)
│  ├─ install-extension.mjs   # CLI: положить extension override в %APPDATA%
│  └─ uninstall-extension.mjs
├─ vite.config.mjs            # renderer / main / preload environments
├─ vite.extensions.config.mjs # билд для extensions/
└─ build/                     # иконки + afterPack hook

extensions/                   # ← top-level рядом с shell/
├─ dashboard/  ├─ delphi/  ├─ horologion/  ├─ arrancador/
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

### Static commands (registry в `shell/electron/commands.ts`)

Каждая запись — `InternalCommand` с полями `id`, `title`, `subtitle`, `category` (`'open' | 'action'`), `kind` (`'app' | 'command'`), `appName?` и `icon?: () => string | undefined`.

| id | kind | title | Что делает |
|---|---|---|---|
| `delphi:open` | `app` | Открыть Delphi | `openExtension("delphi")` |
| `horologion:open` | `app` | Открыть Horologion | `openExtension("horologion")` |
| `arrancador:open` | `app` | Открыть Arrancador | `openExtension("arrancador")` |
| `dashboard:open` | `command` (`appName: "Kepler"`) | Открыть таблицу данных | `openDashboardWindow()` |
| `settings:open` | `command` (`appName: "Kepler"`) | Открыть настройки | `openSettings()` |
| `kepler:check-updates` | `command` (`appName: "Kepler"`) | Проверить обновления | Открывает Settings + `autoupdater.check()` |
| `delphi:today` | `command` (`appName: "Delphi"`) | Сегодняшние задачи | `openExtension("delphi", "/today")` |
| `horologion:pomodoro` | `command` (`appName: "Horologion"`) | Помодоро | `openExtension("horologion", "/?mode=pomodoro")` |
| `horologion:stopwatch` | `command` (`appName: "Horologion"`) | Секундомер | `openExtension("horologion", "/?mode=stopwatch")` |

::: info Eden команда удалена
До 2026-05-14 в registry был `eden:open`, запускавший standalone Eden .exe. После brand swap'а удалена — Eden остаётся standalone, но не показывается в launcher'е до миграции в extension (Phase 6).
:::

::: info `mode=` hash для Horologion — TODO
Команды `horologion:pomodoro` / `horologion:stopwatch` грузят extension с hash `/?mode=pomodoro` / `/?mode=stopwatch`, но HomeView пока не разбирает параметр — переключение режима будет в следующей итерации. Сама команда уже работает: extension открывается на правильном route.
:::

### Dynamic (action) commands

Регистрируются running extension'ами через [Command bus](../concepts/command-bus.md) (`category: 'action'`). LauncherView их **не показывает** (фильтр `category !== "action"`) — палитра ограничена open-командами. Action-команды используются программным вызовом из других мест.

## Extension host (Phase 4 ✅)

`shell/electron/extension-host.ts` — production loader. Phase 4 завершён: 4 апки рендерятся как Vue extensions внутри Kepler без отдельных Electron-процессов.

- Extensions лежат в `extensions/<id>/` (top-level, рядом с `shell/`): Dashboard, Horologion, Delphi, Arrancador.
- Каждое — `manifest.json` + Vue bundle + опциональный preload.
- Host открывает extension в отдельном `BrowserWindow` с reuse через `Map<id, BrowserWindow>`.
- Eden — намеренно standalone .exe (`apps/eden/ts/`), миграция в Phase 6, см. [Roadmap](./kepler-roadmap.md).

### Иконки в launcher

Open-command extension'а показывает PNG иконку соответствующего extension'а. `shell/electron/extension-host.ts → extensionIconDataUri(id)` читает `extensions/<id>/icon.png`, кодирует в data-uri и кэширует **по mtime файла**: при изменении иконки на диске cache автоматически инвалидируется (hot-swap без перезапуска Kepler).

Builtin Kepler-команды без extension PNG (`settings:open`, `dashboard:open`, `kepler:check-updates`) рендерятся через `BuiltInIcon.vue` — gradient-плашка с Lucide-иконкой. Реестр `BUILTIN_ICONS` живёт в `LauncherView.vue` и мапит command id → `{icon, from, to}`: сейчас `settings:open` — серый (`Settings`), `dashboard:open` — teal (`Database`). Команда без записи в реестре получает дефолт компонента — голубой→синий + `HelpCircle`.

`BuiltInIcon` принимает пропсы `icon` (Lucide component), `from` / `to` (gradient stops, OKLCH), `size`, `strokeWidth`. Используется только в launcher; для extension PNG по-прежнему `<img src="data:..." />`.

### Секции и история выбора в launcher

LauncherView показывает две секции, когда строка поиска пустая:

- **Недавние** — до 5 последних invoke'нутых команд в LRU-порядке. Хранится в `localStorage` под ключом `kepler.launcher.recents` (массив id-строк). Запись делается на каждом `invokeSelected()` через `recordRecent(id)`.
- **Все** — полный список команд в порядке регистрации в `COMMANDS` + dynamic поверх (после фильтра `category !== "action"`).

При непустой строке секции скрываются — показывается единый fuzzy-отсортированный список.

`CommandRecord` (`shell/shared/ipc-types.ts`) расширен полями `kind: 'app' | 'command'` и `appName?`. UI рендерит правую часть row'а так:
- `kind === 'app'` → лейбл «Приложение».
- `kind === 'command'` → приглушённое имя приложения справа от title + лейбл «Команда».

### Crash safety

`BrowserWindow.on("closed", …)` обращается к `win.webContents.id` **после** destroy и крашит процесс. Фикс: `wcId` захватывается **до** регистрации listener'ов (`const wcId = win.webContents.id; win.on("closed", () => webContentsToExtensionId.delete(wcId))`). Без этого Kepler падал при закрытии extension-окна.

RAM-эффект: −124 MB Working Set / −209 MB Private Bytes / −4 процесса (см. [RAM benchmarks](../concepts/ram-benchmarks.md)).

Developer mode с Vite HMR per extension — [Extension dev mode](../concepts/extension-dev-mode.md). Полная архитектура — [Extension host](../concepts/extension-host.md).

## Окно настроек

Настройки открываются как **отдельное `BrowserWindow`** через IPC `kepler:settings:open` (паттерн как в [Horologion](./horologion.md#окно-настроек)). Хеш-route `#/settings`, App.vue рендерит `SettingsView` внутри `<DesktopChrome>`. Размер окна — **880×560**, resizable (чтобы Extensions-таб с длинным списком и каталогом помещался без скролла).

В Settings:

- **Autostart** toggle — пишет / удаляет ключ `Kepler` в `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` через `app.setLoginItemSettings`. Ошибки записи показываются inline («Ошибка записи в реестр»).
- **Backend status** — состояние `kepler-backend.exe` child process'а (running / not running) + текущий порт WS.
- **Версия** — `app.getVersion()`.
- **Developer Mode** toggle (Phase 5) — persist'ится в `%APPDATA%\Kosmos\kepler-shell-settings.json`. Когда включён, extension-host резолвит `loadURL('http://localhost:<devPort>/')` вместо `loadFile(dist/...)` для extension'ов, у которых в `manifest.json` указан `devPort`. F12 в любом extension window открывает DevTools.
- **Update banner** (sticky 32px вверху Settings окна) — Raycast-style индикатор состояния autoUpdater'а. Состояния `available` / `downloading` (progress bar) / `downloaded` (click-to-install) / `error`. Иконки `ArrowUpCircle` / `Loader2` из `lucide-vue-next`. Подписан на `window.kepler.settings.update.onStateChanged(...)`. См. [Distribution → autoUpdater](../concepts/distribution.md#kepler-launcher-autoupdater).
- **Кнопка «Проверить обновления»** (General tab) — вызывает `window.kepler.settings.update.check()`. Тот же эффект, что launcher-команда `kepler:check-updates`.
- **Расширения** (таб) — плоский список установленных. Для каждого extension'а:
  - lookup в каталоге: если `catalog.version > installed.version` → кнопка **«Обновить»**;
  - кнопка **«Откатить»** (если есть backup в `extensions-backups/<id>/`);
  - кнопка **«Удалить»**.
  - Сверху таба — кнопка **«Проверить обновления»** (вызывает `loadCatalog(force=true)`).
  - Отдельной секции «Каталог» / «Прочие установленные» больше нет: каталог сейчас используется только для lookup версий, не как картинная витрина.

## Selected space DB resolution

`kepler-backend` по умолчанию пишет в `%APPDATA%\Kosmos\ark.db`, но реальные данные пользователя живут в **выбранном space'е** — `%APPDATA%\Kosmos\spaces\<spaceId>\ark.db`. Чтобы backend читал правильную базу, Kepler shell резолвит путь сам:

1. `shell/electron/main.ts → resolveSpaceDbPath()` читает `%APPDATA%\Kosmos\selected-space.json` через хелперы `readSharedSelectedSpace` / `getArkDbPathForSelectedSpace` из [`@kepler/ark`](../packages/ark.md).
2. Если space выбран — `spawnBackend()` передаёт env-переменную `KOSMOS_DB_PATH=<spaceDir>/ark.db` дочернему процессу `kepler-backend.exe`.
3. Backend использует `KOSMOS_DB_PATH` вместо дефолта.
4. Если файла `selected-space.json` нет / он битый — backend падает на default. Это нормальное поведение для свежей инсталляции до создания первого space'а.

## Production packaging (Phase 8)

`bun run build` собирает финальный **NSIS one-click** установщик через electron-builder. Конфиг — в `shell/package.json → build`.

Pipeline:

1. `build:backend` — `cargo build --release --bin kepler-backend`.
2. `build:js` — tsc + vite build (main + preload + renderer).
3. `electron-builder --win nsis` — `release/Kepler Setup X.Y.Z.exe`.

`extraResources` (копируются рядом с упакованным `Kepler.exe`):

- `kepler-backend.exe` — Rust backend (из `services/kepler-backend/target/release/`).
- `ark-core-rpc.exe` — для legacy standalone-апок, которые ещё не extensions.
- `extensions/` — bundle'ы Vue extensions (только `manifest.json`, `icon.png`, `index.html`, `dist/`; исключаются `src/`, `node_modules/`, `package.json`, vite configs).
- `icon.png` — для tray и `BrowserWindow.icon`.

`afterPack` (`build/afterPack.cjs`) embed'ит иконку в `Kepler.exe` через `rcedit` + `png-to-ico` (тот же паттерн, что у [Horologion](./horologion.md) / [Delphi](./delphi.md) — workaround под отключённый встроенный rcedit electron-builder из-за `win.signAndEditExecutable: false`).

NSIS-настройки: `oneClick: true`, `perMachine: false` (install в `%LocalAppData%\Kepler` без UAC), `runAfterFinish: true`, desktop + Start Menu shortcut, `deleteAppDataOnUninstall: false` (не теряем space-данные при апдейте).

Что ещё в Phase 8 (⏳): `electron-updater` для auto-update. Legacy Rust gpui launcher уже удалён в Phase A (директория apps/kepler/ больше не существует).

## Extension installer (Phase 10 MVP)

Built-ins (Horologion, Delphi, Arrancador) едут с Kepler installer'ом в `<resourcesPath>/extensions/`. Поверх можно положить свежую копию extension'а в `%APPDATA%\Kosmos\extensions\<id>\` — resolution chain в `extension-host.ts` ставит её выше bundled, перекрывая для этого `id`. Удаление user-папки откатывает на bundled.

Dashboard в этот список **не входит** — после 2026-05-14 он встроенный shell view (см. [Dashboard](/apps/dashboard)), не extension.

```powershell
# install: <path-to-extension-dir> должен содержать manifest.json, dist/, icon.png
bun run --cwd shell ext:install ./extensions/horologion
# uninstall
bun run --cwd shell ext:uninstall horologion
```

Подробно (atomic копирование, layout, что НЕ входит в MVP — auto-update, `.kext` формат, UI manager) — [Extension installer](../concepts/extension-installer.md).

## Запуск (dev)

```powershell
cd shell
bun run build:backend:dev   # cargo build (debug) services/kepler-backend
bun run dev                 # build:backend:dev + extensions + vite + Electron
```

`Ctrl+Shift+K` глобально откроет launcher. Tray-иконка появится в трее.

## Команды

| Команда | Что |
|---|---|
| `bun run --cwd shell dev` | dev режим |
| `bun run --cwd shell build:js` | tsc + vite build (без NSIS) |
| `bun run --cwd shell build` | release backend + js + NSIS installer |
| `bun run --cwd shell typecheck` | tsc --noEmit |
| `bun run --cwd shell package:dir` | unpacked Electron сборка |
| `bun run --cwd shell test:e2e` | Playwright e2e |
| `bun run --cwd shell ext:install <path>` | поставить extension в `%APPDATA%\Kosmos\extensions\<id>\` (override bundled). См. [Extension installer](../concepts/extension-installer.md). |
| `bun run --cwd shell ext:uninstall <id>` | удалить user-installed extension; bundled (если есть) поднимется автоматически. |

Артефакты `build` — `shell/release/Kepler Setup X.Y.Z.exe` (NSIS one-click).

## История legacy Rust-launcher'а

Старый Rust-launcher (gpui + tao + global-hotkey) лежал в apps/kepler и удалён в Phase A (после brand swap'а 2026-05-14). Вся работа идёт через `shell/`.

## См. также

- [Roadmap](./kepler-roadmap.md) — фазы миграции и план Phase 4-6.
- [Command bus](../concepts/command-bus.md) — протокол dynamic commands.
- [@kepler/ark](../packages/ark.md) — TS SDK с `commands` namespace.
- [Architecture](../concepts/architecture.md) — общая картина Kosmos.
