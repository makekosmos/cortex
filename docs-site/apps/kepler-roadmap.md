# Kepler — Roadmap

Pivot 2026-05-14: ecosystem `Kepler` → `Kosmos`, launcher `Kosmos` → `Kepler`. Все паттерны swap'нуты в коммите brand-swap. Эта страница — единый план работ по Electron host'у `apps/kepler-shell/`.

## Статус по фазам

| Фаза | Что | Статус |
|---|---|---|
| 0 | Backend extracted в `services/kepler-backend/` (lib + bin `kepler-backend.exe`) | ✅ |
| 1 | Electron shell scaffold (launcher window, tray, settings, hotkey, backend spawn, window state) | ✅ |
| 2 | Command bus (Rust в backend + `@kosmos/ark` SDK + apps register + dynamic launcher) | ✅ |
| 3 | Real handlers (Horologion / Delphi / Eden wired), Settings window, extension loader PoC | ✅ |
| 4 | Apps как Vue extensions внутри Kepler | ⏳ |
| 5 | Adaptive lifecycle (optional) | ⏳ |
| 6 | Retire legacy Rust `apps/kepler/`, production packaging, auto-update | ⏳ |

## Phase 0 ✅ — Backend extracted

Из старого `apps/kepler/` (Rust gpui launcher) выделен чистый backend в `services/kepler-backend/`:
- Crate с lib (ARK runtime, WS server, command bus, sync) + бинарь `kepler-backend.exe`.
- Spawn'ится Electron host'ом как child process; держит ARK и обслуживает WS-клиентов.
- Сборка: `cargo build --release --manifest-path services/kepler-backend/Cargo.toml --bin kepler-backend`.

## Phase 1 ✅ — Electron shell scaffold

`apps/kepler-shell/` — новая Electron-апка:
- Frameless 720×460 launcher window, Mica/Acrylic (Win11), centered на active display, `nativeTheme.themeSource = 'dark'`.
- `globalShortcut Ctrl+Shift+K` toggle show/hide; при потере фокуса — hide.
- Tray icon с меню Открыть / Выйти.
- Backend spawn (resolveBackendExe → debug или extraResource), graceful shutdown.
- Singleton lock (`app.requestSingleInstanceLock`).
- Window state persistence (под user data).
- Settings — отдельное `BrowserWindow` через IPC `kepler:settings:open`.

## Phase 2 ✅ — Command bus

Полноценный dynamic command flow:
- `kepler-backend` Rust: in-memory registry, WS-операции `commands.{register,unregister,list,invoke}`, события `command_invoked` / `commands_changed`.
- `@kosmos/ark` TypeScript: `ArkClient.commands` namespace + типизированные payloads + event subscription.
- Электронные апки регистрируют команды при старте; backend роутит invoke к нужному client'у через события.
- `LauncherView.vue` слушает `commands_changed`, рендерит filtered список, при выборе делает invoke.

Подробно — [Command bus](../concepts/command-bus.md).

## Phase 3 ✅ — Real handlers + extension PoC

Динамические команды реально что-то делают:
- **Horologion**: `horologion:pomodoro:25`, `horologion:pomodoro:50`, `horologion:stopwatch:start`. Main → IPC `horologion:cmd` → renderer вызывает `pomodoro.start({ workMinOverride })` или `timeEntries.startTimer`. `usePomodoro` поддерживает `workMinOverride` для per-session override без мутации persistent settings.
- **Delphi**: `delphi:task:create` (открывает QuickEntry) и `delphi:task:today` (router.push '/today'). `SidecarClient.onCommand` listener + `focusMainWindow` перед dispatch.
- **Eden**: `eden:note:create` (новая заметка), `eden:search` (overlay поиска через FTS5).

Дополнительно:
- Settings window для Kepler shell (отдельный `BrowserWindow`, hash `#/settings`).
- Extension loader PoC: `electron/extension-host.ts` загружает static extensions из `apps/kepler-shell/extensions/<id>/{manifest.json,index.html,bundle.js}` в отдельные BrowserWindow'ы. Демо: `extensions/dashboard/`.

## Phase 4 ⏳ — Apps как Vue extensions

Цель: рендерить Kosmos-апки **внутри** Kepler как extensions, без отдельных Electron-процессов.

План:
1. **Dashboard** — первый кандидат (read-only, минимум IPC, существующий PoC). Реальная миграция Vue app → extension bundle.
2. **Horologion** — после Dashboard, как ext bundle.
3. **Delphi** — следующий кандидат.
4. **Eden** — последний (самый сложный, Heart vault).

Требует: extension manifest spec, sandbox для preload API, dynamic loading в LauncherView, общий ARK access через `window.kepler.ark.*`.

## Phase 5 ⏳ — Adaptive lifecycle (optional)

Динамическое включение/выключение extensions на основе usage (LRU eviction, RAM budget). Зависит от Phase 4.

## Phase 6 ⏳ — Retire legacy + packaging

- Удаление `apps/kepler/` (старый Rust gpui launcher).
- Production NSIS packaging Kepler shell с включённым `kepler-backend.exe` + `ark-core-rpc.exe` через `extraResources`.
- Auto-update mechanism (electron-updater).
- Установщик переписывает HKCU Run на новый `Kepler.exe`.

## Баги / замечания

- ⚠️ **Win32 SetWindowPos jitter** при show/hide launcher окна — зафиксили: snap resize + GPU CSS Transition. Если регрессии в Phase 4 (extension windows) — смотреть туда же.
