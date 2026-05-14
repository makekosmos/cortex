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
- Tray icon с меню «Открыть» / «Выйти». Реальный quit — только через tray.

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

RAM-эффект: −124 MB Working Set / −209 MB Private Bytes / −4 процесса (см. [RAM benchmarks](../concepts/ram-benchmarks.md)).

Developer mode с Vite HMR per extension — [Extension dev mode](../concepts/extension-dev-mode.md). Полная архитектура — [Extension host](../concepts/extension-host.md).

## Окно настроек

Настройки открываются как **отдельное `BrowserWindow`** через IPC `kepler:settings:open` (паттерн как в [Horologion](./horologion.md#окно-настроек)). Хеш-route `#/settings`, App.vue рендерит `SettingsView` внутри `<DesktopChrome>`.

В Settings — статус `kepler-backend`, путь к ARK DB, hotkey, autostart toggle.

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

Артефакты `build` — `apps/kepler-shell/release/Kepler Setup X.Y.Z.exe` (NSIS one-click).

## Legacy Rust `apps/kepler/`

Старый Rust-launcher (gpui + tao + global-hotkey) ещё лежит в `apps/kepler/`, но **retired**: вся новая работа идёт через `apps/kepler-shell/`. Phase 6 убирает legacy директорию целиком — см. [Roadmap](./kepler-roadmap.md).

## См. также

- [Roadmap](./kepler-roadmap.md) — фазы миграции и план Phase 4-6.
- [Command bus](../concepts/command-bus.md) — протокол dynamic commands.
- [@kosmos/ark](../packages/kosmos-ark.md) — TS SDK с `commands` namespace.
- [Architecture](../concepts/architecture.md) — общая картина Kosmos.
