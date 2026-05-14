# Kepler — Roadmap

Pivot 2026-05-14: ecosystem `Kepler` → `Kosmos`, launcher `Kosmos` → `Kepler`. Все паттерны swap'нуты в коммите brand-swap. Эта страница — единый план работ по Electron host'у `apps/kepler-shell/`.

## Статус по фазам

| Фаза | Что | Статус |
|---|---|---|
| 0 | Backend extracted в `services/kepler-backend/` (lib + bin `kepler-backend.exe`) | ✅ |
| 1 | Electron shell scaffold (launcher window, tray, settings, hotkey, backend spawn, window state) | ✅ |
| 2 | Command bus (Rust в backend + `@kosmos/ark` SDK + apps register + dynamic launcher) | ✅ |
| 3 | Real handlers (Horologion / Delphi / Eden wired), Settings window, extension loader PoC | ✅ |
| 4 | Apps как Vue extensions внутри Kepler (Dashboard / Horologion / Delphi / Arrancador) | ✅ |
| 5 | Extension developer mode (Vite HMR per extension, Raycast-style) | ⏳ |
| 6 | Eden как extension (намеренно отложено) | ⏳ |
| 7 | Adaptive lifecycle (optional) | ⏳ |
| 8 | Retire legacy Rust `apps/kepler/`, production packaging, auto-update | ⏳ |

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

## Phase 4 ✅ — Apps как Vue extensions

Цель достигнута: 4 апки рендерятся **внутри** Kepler как Vue extensions, без отдельных Electron-процессов. Eden намеренно остался standalone (см. Phase 6).

Мигрированы:

- **Dashboard** — полная Vue migration, build ~83 KB JS. Read-only аналитика, ARK через preload bridge.
- **Horologion** — полная Vue migration с `horologionApi` shim над `window.kepler.*`. Build ~102 KB chunk `pomodoroSettings`. Pomodoro/stopwatch state работает.
- **Delphi** — Vue + memory router, build 3483 modules. После Phase 5 cleanup: `electron-api-shim.ts` устанавливает `window.electronAPI` поверх `kepler.ark.request` — все existing call sites работают. Tailwind plugin подключён (Phase 5). **Открытый вопрос** — переписать Delphi UI с Tailwind utility classes на plain CSS + `@kosmos/visuals` tokens (как остальные extension'ы). См. Phase 9 ниже.
- **Arrancador** — UI subset (LayoutPage + GameCard). Catalogue / Scan / Sqoba / Stats / Settings pages **не мигрированы** — native scanner остаётся в legacy standalone .exe.

RAM-эффект Phase 4 — −124 MB Working Set / −209 MB Private Bytes / −4 процесса. Полная таблица — [RAM benchmarks](/concepts/ram-benchmarks).

## Phase 5 ⏳ — Extension developer mode

Hot-reload для extensions через Vite dev servers, как `ray develop` у Raycast. Подробно — [Extension dev mode](/concepts/extension-dev-mode).

Состав:

- `bun run --cwd apps/kepler-shell dev:extensions` поднимает Vite dev server на отдельном порту для каждого extension'а (5180–5183).
- `KEPLER_DEV=1` + поле `devPort` в manifest → extension-host резолвит `loadURL('http://localhost:<port>/')` вместо `loadFile(dist/...)`.
- Settings → Developer Mode toggle (persist в `%APPDATA%\Kosmos\kepler-shell-settings.json`).
- F12 toggles DevTools на любом extension window.

Также под Phase 5: cleanup `electronAPI` хвостов в Delphi extension и доделка остальных страниц Arrancador.

## Phase 6 ⏳ — Eden как extension

Eden — самый сложный кейс (TipTap editor + Heart Rust поиск + широкий preload API: titlebar history, store hardening, FTS, vault). Намеренно отложено пока остальные апки в extensions стабилизируются. Ожидаемый RAM-эффект — ~250 MB save относительно Eden.exe standalone.

## Phase 7 ⏳ — Adaptive lifecycle (optional)

Динамическое включение/выключение extensions на основе usage (LRU eviction, RAM budget). Зависит от Phase 4-6.

## Phase 8 ⏳ — Retire legacy + packaging

- Удаление `apps/kepler/` (старый Rust gpui launcher).
- Production NSIS packaging Kepler shell с включённым `kepler-backend.exe` + `ark-core-rpc.exe` через `extraResources`.
- Auto-update mechanism (electron-updater).

## Phase 9 ⏳ — Delphi UI: Tailwind → plain CSS (открытый вопрос)

Delphi extension сейчас использует Tailwind v4 в templates (наследие legacy `apps/delphi/ts/`). Все остальные extension'ы (Horologion, Dashboard, Arrancador, Kepler launcher / settings) написаны на **plain scoped CSS + `@kosmos/visuals` CSS variables** — единый стиль через ecosystem.

Что нужно для Phase 9:

- Пройти ~30 .vue файлов в `apps/kepler-shell/extensions/delphi/src/{App.vue, components, pages}`.
- Удалить Tailwind utility classes (`flex items-center gap-2 px-3 rounded-lg ...`) из шаблонов.
- Переписать стили в `<style scoped>` с `var(--background) / --foreground / --border / --radius-*` из `@kosmos/visuals`.
- Удалить `@import "tailwindcss"` + `@source` directive из `src/global.css`.
- Удалить `@tailwindcss/vite` plugin из `extensions/delphi/vite.config.mjs` + `vite.extensions.config.mjs`.
- Удалить `tailwindcss` + `@tailwindcss/vite` deps из `extensions/delphi/package.json` (и shared kepler-shell deps если нет других пользователей).
- Verify build + manual UI smoke (Delphi выглядит OK на acrylic Mica background).

Скоуп — несколько часов сфокусированной работы (или один agent). Откладываем до момента когда Delphi UI стабилизируется (project / area / settings flows не меняются часто) — иначе придётся переделывать дважды.
- Установщик переписывает HKCU Run на новый `Kepler.exe`.

## Баги / замечания

- ⚠️ **Win32 SetWindowPos jitter** при show/hide launcher окна — зафиксили: snap resize + GPU CSS Transition. Если регрессии в Phase 4 (extension windows) — смотреть туда же.
