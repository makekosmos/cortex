# Kepler — Electron host и global launcher

::: tip Источник правды
`apps/kepler-shell/`, `services/kepler-backend/`, `.agent/tasks/2026-05-14-kosmos-pivot/`
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

Каждая Electron-апка коннектится к `kepler-backend` через WebSocket (`@kosmos/ark` kepler mode), регистрирует свои команды через [Command bus](../concepts/command-bus.md) и слушает события `command_invoked`. Когда юзер открывает Kepler launcher и выбирает команду — backend роутит её к нужной апке.

## Стек

| Слой | Технология |
|---|---|
| Shell | Electron 41 (frameless, Mica/Acrylic, transparent) |
| Renderer | Vue 3.6 + TypeScript + Vite 8 (electron-vite) |
| Bundler | `vite-plugin-electron` + `vite-plugin-electron-renderer` |
| Backend | `kepler-backend.exe` (Rust, lib + bin из `services/kepler-backend/`) |
| ARK SDK | `@kosmos/ark` (kepler mode, hello-handshake, command bus client) |
| UI | `@kosmos/visuals` (DesktopChrome, токены, компоненты) |
| Tray / hotkey | Electron `Tray` + `globalShortcut` |

## Структура

```
apps/kepler-shell/
├─ electron/
│  ├─ main.ts              # backend spawn, BrowserWindow, tray, globalShortcut, IPC
│  ├─ preload.ts           # window.kepler API (search / invoke / commands)
│  ├─ commands.ts          # статические команды (kepler:open-app:*, dashboard:open, …)
│  ├─ settings-window.ts   # отдельное окно настроек + IPC handlers
│  └─ extension-host.ts    # PoC загрузчик static extensions
├─ shared/
│  └─ ipc-types.ts         # KeplerApi (preload contract), CommandRecord, SearchResult
├─ src/
│  ├─ App.vue              # routing (LauncherView / SettingsView по hash)
│  ├─ main.ts              # createApp + Inter Variable
│  ├─ styles.css           # --kepler-accent + локальные токены
│  └─ views/
│     ├─ LauncherView.vue  # FTS5 search + dynamic command list
│     └─ SettingsView.vue  # настройки host'а (hotkey, backend status)
├─ extensions/
│  └─ dashboard/           # PoC static extension (manifest.json + index.html + bundle.js)
└─ build/                  # иконки + afterPack hook
```

## Окно launcher'а

- 720×460 fixed, не resizable, frameless, transparent.
- `backgroundMaterial: 'mica'` (Win11) / acrylic fallback на старых билдах.
- Центрируется на active display.
- `nativeTheme.themeSource = 'dark'` — принудительно тёмная тема, независимо от системы.
- `globalShortcut.register('Ctrl+Shift+K')` (`Cmd+Shift+K` на macOS) — toggle show/hide.
- При потере фокуса — окно скрывается (focus-trap pattern, как Spotlight).
- Tray icon с меню **«Открыть» / «Настройки» / «Выход»**. «Настройки» открывает [отдельное окно настроек](#окно-настроек) через тот же IPC `kepler:settings:open`. Реальный quit — только через tray «Выход».

## Command bus

Kepler — точка входа для всех команд экосистемы. Подробно см. [Command bus](../concepts/command-bus.md).

Коротко:
- `kepler-backend` хранит in-memory registry команд (`commands.register/unregister/list/invoke`) и эмитит события `command_invoked` / `commands_changed` через WS.
- Электронные апки при старте делают `ArkClient.commands.register([...])` и подписываются на `command_invoked` события для своих id'шников.
- `LauncherView` слушает `commands_changed`, держит актуальный список и при выборе вызывает `commands.invoke(id)`.
- Статические команды (`dashboard:open`, `arrancador:open`, `kepler:settings:open`) живут в `electron/commands.ts` и матчатся локально без backend roundtrip.

Зарегистрированные сейчас динамические команды (Phase 3):
- **Horologion** — `horologion:pomodoro:25`, `horologion:pomodoro:50`, `horologion:stopwatch:start`.
- **Delphi** — `delphi:task:create`, `delphi:task:today`.
- **Eden** — `eden:note:create`, `eden:search`.

## Extension host (Phase 4 ✅)

`electron/extension-host.ts` — production loader. Phase 4 завершён: 4 апки рендерятся как Vue extensions внутри Kepler без отдельных Electron-процессов.

- Extensions лежат в `apps/kepler-shell/extensions/<id>/` (Dashboard, Horologion, Delphi, Arrancador).
- Каждое — `manifest.json` + Vue bundle + опциональный preload.
- Host открывает extension в отдельном `BrowserWindow` с reuse через `Map<id, BrowserWindow>`.
- Eden — намеренно standalone .exe (миграция в Phase 6, см. [Roadmap](./kepler-roadmap.md)).

### Иконки в launcher

Каждое open-command в launcher показывает иконку соответствующего extension'а. `electron/extension-host.ts → extensionIconDataUri(id)` читает `extensions/<id>/icon.png`, кодирует в data-uri и кэширует **по mtime файла**: при изменении иконки на диске cache автоматически инвалидируется (hot-swap без перезапуска Kepler). Если иконки нет — команда показывается без неё. Eden команда **без иконки** (legacy, ещё не extension — см. Phase 6).

### Crash safety

`BrowserWindow.on("closed", …)` обращается к `win.webContents.id` **после** destroy и крашит процесс. Фикс: `wcId` захватывается **до** регистрации listener'ов (`const wcId = win.webContents.id; win.on("closed", () => webContentsToExtensionId.delete(wcId))`). Без этого Kepler падал при закрытии extension-окна.

RAM-эффект: −124 MB Working Set / −209 MB Private Bytes / −4 процесса (см. [RAM benchmarks](../concepts/ram-benchmarks.md)).

Developer mode с Vite HMR per extension — [Extension dev mode](../concepts/extension-dev-mode.md). Полная архитектура — [Extension host](../concepts/extension-host.md).

## Окно настроек

Настройки открываются как **отдельное `BrowserWindow`** через IPC `kepler:settings:open` (паттерн как в [Horologion](./horologion.md#окно-настроек)). Хеш-route `#/settings`, App.vue рендерит `SettingsView` внутри `<DesktopChrome>`.

В Settings:

- **Autostart** toggle — пишет / удаляет ключ `Kepler` в `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` через `app.setLoginItemSettings`. Ошибки записи показываются inline («Ошибка записи в реестр»).
- **Backend status** — состояние `kepler-backend.exe` child process'а (running / not running) + текущий порт WS.
- **Версия** — `app.getVersion()`.
- **Developer Mode** toggle (Phase 5) — persist'ится в `%APPDATA%\Kosmos\kepler-shell-settings.json`. Когда включён, extension-host резолвит `loadURL('http://localhost:<devPort>/')` вместо `loadFile(dist/...)` для extension'ов, у которых в `manifest.json` указан `devPort`. F12 в любом extension window открывает DevTools.

## Selected space DB resolution

`kepler-backend` по умолчанию пишет в `%APPDATA%\Kosmos\ark.db`, но реальные данные пользователя живут в **выбранном space'е** — `%APPDATA%\Kosmos\spaces\<spaceId>\ark.db`. Чтобы backend читал правильную базу, Kepler shell резолвит путь сам:

1. `electron/main.ts → resolveSpaceDbPath()` читает `%APPDATA%\Kosmos\selected-space.json` через хелперы `readSharedSelectedSpace` / `getArkDbPathForSelectedSpace` из [`@kosmos/ark`](../packages/kosmos-ark.md).
2. Если space выбран — `spawnBackend()` передаёт env-переменную `KOSMOS_DB_PATH=<spaceDir>/ark.db` дочернему процессу `kepler-backend.exe`.
3. Backend использует `KOSMOS_DB_PATH` вместо дефолта.
4. Если файла `selected-space.json` нет / он битый — backend падает на default. Это нормальное поведение для свежей инсталляции до создания первого space'а.

## Production packaging (Phase 8)

`bun run build` собирает финальный **NSIS one-click** установщик через electron-builder. Конфиг — в `apps/kepler-shell/package.json → build`.

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

Что ещё в Phase 8 (⏳): `electron-updater` для auto-update и удаление legacy `apps/kepler/` (старый Rust gpui launcher).

## Extension installer (Phase 10 MVP)

Built-ins (Dashboard, Horologion, Delphi, Arrancador) едут с Kepler installer'ом в `<resourcesPath>/extensions/`. Поверх можно положить свежую копию extension'а в `%APPDATA%\Kosmos\extensions\<id>\` — resolution chain в `extension-host.ts` ставит её выше bundled, перекрывая для этого `id`. Удаление user-папки откатывает на bundled.

```powershell
# install: <path-to-extension-dir> должен содержать manifest.json, dist/, icon.png
bun run --cwd apps/kepler-shell ext:install ./apps/kepler-shell/extensions/dashboard
# uninstall
bun run --cwd apps/kepler-shell ext:uninstall dashboard
```

Подробно (atomic копирование, layout, что НЕ входит в MVP — auto-update, `.kext` формат, UI manager) — [Extension installer](../concepts/extension-installer.md).

## Запуск (dev)

```powershell
cd apps/kepler-shell
bun run build:backend:dev   # cargo build (debug) services/kepler-backend
bun run dev                 # build:backend:dev + vite + Electron
```

`Ctrl+Shift+K` глобально откроет launcher. Tray-иконка появится в трее.

## Команды

| Команда | Что |
|---|---|
| `bun run --cwd apps/kepler-shell dev` | dev режим |
| `bun run --cwd apps/kepler-shell build:js` | tsc + vite build (без NSIS) |
| `bun run --cwd apps/kepler-shell build` | release backend + js + NSIS installer |
| `bun run --cwd apps/kepler-shell typecheck` | tsc --noEmit |
| `bun run --cwd apps/kepler-shell package:dir` | unpacked Electron сборка |
| `bun run --cwd apps/kepler-shell test:e2e` | Playwright e2e |
| `bun run --cwd apps/kepler-shell ext:install <path>` | поставить extension в `%APPDATA%\Kosmos\extensions\<id>\` (override bundled). См. [Extension installer](../concepts/extension-installer.md). |
| `bun run --cwd apps/kepler-shell ext:uninstall <id>` | удалить user-installed extension; bundled (если есть) поднимется автоматически. |

Артефакты `build` — `apps/kepler-shell/release/Kepler Setup X.Y.Z.exe` (NSIS one-click).

## Legacy Rust `apps/kepler/`

Старый Rust-launcher (gpui + tao + global-hotkey) ещё лежит в `apps/kepler/`, но **retired**: вся новая работа идёт через `apps/kepler-shell/`. Phase 6 убирает legacy директорию целиком — см. [Roadmap](./kepler-roadmap.md).

## См. также

- [Roadmap](./kepler-roadmap.md) — фазы миграции и план Phase 4-6.
- [Command bus](../concepts/command-bus.md) — протокол dynamic commands.
- [@kosmos/ark](../packages/kosmos-ark.md) — TS SDK с `commands` namespace.
- [Architecture](../concepts/architecture.md) — общая картина Kosmos.
