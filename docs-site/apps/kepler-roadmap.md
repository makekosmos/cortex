# Kepler — Roadmap

Pivot 2026-05-14: ecosystem `Kepler` → `Kosmos`, launcher `Kosmos` → `Kepler`. Все паттерны swap'нуты в коммите brand-swap. Эта страница — единый план работ по Electron host'у `shell/`.

## Статус по фазам

| Фаза | Что | Статус |
|---|---|---|
| 0 | Backend extracted в `services/kepler-backend/` (lib + bin `kepler-backend.exe`) | ✅ |
| 1 | Electron shell scaffold (launcher window, tray, settings, hotkey, backend spawn, window state) | ✅ |
| 2 | Command bus (Rust в backend + `@kepler/ark` SDK + apps register + dynamic launcher) | ✅ |
| 3 | Real handlers (Horologion / Delphi / Eden wired), Settings window, extension loader PoC | ✅ |
| 4 | Apps как Vue extensions внутри Kepler (Dashboard / Horologion / Delphi / Arrancador) | ✅ |
| 5 | Extension developer mode (Vite HMR per extension, Raycast-style) | ✅ |
| 6 | Eden как extension (Phase 6.0 scaffold + ARK note CRUD; Phase 6.0.A cleanup — Hevy/code-tools/standalone удалены, trash UI, codesplit) | ✅ |
| 7 | Universal per-type data export (notes → md, runs → GPX+zip, …) — pipeline в Kepler shell | ⏳ |
| 7.5 | Adaptive lifecycle (optional) | ⏳ |
| 8 | Production packaging (NSIS) ✅ / auto-update ⏳ / retire legacy Rust gpui launcher ✅ | ⏳ |
| 9 | Delphi UI: Tailwind → plain CSS (открытый вопрос) | ⏳ |
| 10 | Extension installer — CLI install/uninstall ✅ / `.kext` ⏳ / UI manager ⏳ / auto-update ⏳ | ⏳ |

## Phase 0 ✅ — Backend extracted

Из старого Rust gpui launcher (легаси, удалён в Phase A) выделен чистый backend в `services/kepler-backend/`:
- Crate с lib (ARK runtime, WS server, command bus, sync) + бинарь `kepler-backend.exe`.
- Spawn'ится Electron host'ом как child process; держит ARK и обслуживает WS-клиентов.
- Сборка: `cargo build --release --manifest-path services/kepler-backend/Cargo.toml --bin kepler-backend`.

## Phase 1 ✅ — Electron shell scaffold

`shell/` — новая Electron-апка:
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
- `@kepler/ark` TypeScript: `ArkClient.commands` namespace + типизированные payloads + event subscription.
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
- Extension loader PoC: `electron/extension-host.ts` загружает static extensions из `extensions/<id>/{manifest.json,index.html,bundle.js}` в отдельные BrowserWindow'ы. Демо: `extensions/horologion/` и др.

## Phase 4 ✅ — Apps как Vue extensions

Цель достигнута: 4 апки рендерятся **внутри** Kepler как Vue extensions, без отдельных Electron-процессов. Eden мигрирован отдельно в Phase 6.0 / 6.0.A (см. ниже).

Мигрированы:

- **Dashboard** — полная Vue migration, build ~83 KB JS. Read-only аналитика, ARK через preload bridge. <span class="kbadge warn">после 2026-05-14 Dashboard rewritten — теперь встроенный shell view (ARK browser), не extension. См. [Dashboard](/apps/dashboard).</span>
- **Horologion** — полная Vue migration с `horologionApi` shim над `window.kepler.*`. Build ~102 KB chunk `pomodoroSettings`. Pomodoro/stopwatch state работает.
- **Delphi** — Vue + memory router, build 3483 modules. После Phase 5 cleanup: `electron-api-shim.ts` устанавливает `window.electronAPI` поверх `kepler.ark.request` — все existing call sites работают. Tailwind plugin подключён (Phase 5). **Открытый вопрос** — переписать Delphi UI с Tailwind utility classes на plain CSS + `@kepler/visuals` tokens (как остальные extension'ы). См. Phase 9 ниже.
- **Arrancador** — UI subset (LayoutPage + GameCard). Catalogue / Scan / Sqoba / Stats / Settings pages **не мигрированы** — native scanner остаётся в legacy standalone .exe.

RAM-эффект Phase 4 — −124 MB Working Set / −209 MB Private Bytes / −4 процесса. Полная таблица — [RAM benchmarks](/concepts/ram-benchmarks).

### Post-migration polish

После основной миграции Phase 4 добавлены доводки, считаются частью Phase 4:

- **App icons в launcher** — каждое open-command (Dashboard / Horologion / Delphi / Arrancador) показывает иконку extension'а в результатах launcher'а. `extensionIconDataUri(id)` в `extension-host.ts` читает `extensions/<id>/icon.png` и кэширует по **mtime файла** — hot-swap иконки без рестарта Kepler. Eden команда без иконки (legacy, не extension).
- **Crash on close fix** — `BrowserWindow.on("closed", …)` теперь использует captured `wcId` (захваченный **до** регистрации listener'а), а не `win.webContents.id` после destroy. До фикса Kepler падал при закрытии extension-окна.
- **Status dot в Horologion topbar** — точка статуса подключения к ARK (probe `list_object_types` каждые 10с), визуально совпадает с Delphi extension'ом.
- **Selected-space DB resolution** — `kepler-shell` main читает `%APPDATA%\Kosmos\selected-space.json` (через `@kepler/ark` хелперы `readSharedSelectedSpace` / `getArkDbPathForSelectedSpace`) и передаёт `KOSMOS_DB_PATH=<spaceDir>/ark.db` в env при `spawnBackend()`. Backend пишет в выбранный space, а не в дефолтный `%APPDATA%\Kosmos\ark.db`.

## Phase 5 ✅ — Extension developer mode

Hot-reload для extensions через Vite dev servers, как `ray develop` у Raycast. Подробно — [Extension dev mode](/concepts/extension-dev-mode).

Состав:

- `bun run --cwd shell dev:extensions` поднимает Vite dev server на отдельном порту для каждого extension'а (5180–5183).
- Settings → Developer Mode toggle (persist в `%APPDATA%\Kosmos\kepler-shell-settings.json`) + поле `devPort` в manifest → extension-host резолвит `loadURL('http://localhost:<port>/')` вместо `loadFile(dist/...)`. `KEPLER_DEV=1` сюда **не** входит — env var управляет только shell-level dev (DevTools шелла), но не extension HMR.
- F12 toggles DevTools на любом extension window.

### Cleanup под Phase 5 (выполнено)

- **Delphi `electron-api-shim`** — `extensions/delphi/src/lib/electron-api-shim.ts` устанавливает `window.electronAPI` поверх `window.kepler.ark.request`, мапит legacy каналы (`ark:listDelphiTasks`, `ark:upsertDelphiTask`, `ark:deleteDelphiTask`, `ark:listTimeEntries`) на ARK operations. LAN sync / P2P / space management — graceful no-op. Подробно — [Delphi → electron-api shim](./delphi.md#electron-api-shim-в-extension).
- **Arrancador pages migration** — все 7 страниц мигрированы в extension (Library, Catalogue, Scan, Sqoba, Statistics, Settings, GameDetail). Native scanner / RAWG / game launch / usage heatmap пока stubs. Подробно — [Arrancador](./arrancador.md).
- **Tailwind restored для Delphi** — `@tailwindcss/vite` plugin подключён обратно в `extensions/delphi/vite.config.mjs`, т.к. оригинальный UI на Tailwind utility classes. Переписывание на plain CSS — открытый вопрос Phase 9.

## Phase 6 ✅ — Eden как extension

Eden — самый сложный кейс (TipTap editor + Heart Rust поиск + широкий preload API: titlebar history, store hardening, FTS, vault).

### Phase 6.0 (2026-05-17) — Scaffold + ARK note CRUD

- `extensions/eden/` создан как Vue extension: manifest (devPort 5184, 1100×750), package.json, vite config, src/ портирован из standalone Eden.
- `kepler-api-shim` (`extensions/eden/src/lib/kepler-api-shim.ts`) эмулирует `window.api` поверх `window.kepler.ark.request(...)`. Шаблон такой же, как Delphi `electron-api-shim` — позволяет сохранить ~50 call-sites Eden codebase'а без переписывания.
- Note CRUD / folders (stubs) / typed-notes / search — через ARK операции (`list_objects`, `get_object`, `upsert_object`, `delete_object`, `list_object_types`, `upsert_object_type`, `search_objects`).
- Команды `eden:note:create`, `eden:note:search` зарегистрированы через command bus.
- Standalone `Eden.exe` остался временно как fallback.

См. `.agent/tasks/2026-05-17-eden-extension/spec.md`.

### Phase 6.0.A (2026-05-17) — Cleanup + hardening

- **Hevy fitness sync удалён.** Будет заменён отдельным приложением Olympia.
- **Code lint/format удалён** (UI + API). Возможно вернётся через child_process capability в shell preload.
- **Trash UI работает** через ARK soft-delete (`deletedAt != null` фильтр + `upsert_object` с `deletedAt: null` для restore + `delete_object` для permanent).
- **Bundle codesplit**: lazy `Editor.vue` через `defineAsyncComponent` → main bundle ~353KB (gzip 112KB) vs prior 1.7MB, editor chunk ~1.36MB загружается при открытии заметки.
- **Standalone `apps/eden/ts/` удалён** полностью. Heart Rust sidecar тоже не нужен (ARK FTS5 покрывает search).
- Workspace cleanup: `package.json` workspaces, `Cargo.toml` exclude, `lefthook.yml` hooks, `scripts/check-ark-write-boundaries.mjs`, `scripts/ark-smoke.mjs`, `scripts/fix-mojibake.mjs`, `scripts/sync-agents-docs.mjs`, `scripts/check-docs-freshness.mjs` — все ссылки на `apps/eden` вычищены.

См. `.agent/tasks/2026-05-17-eden-cleanup-and-hardening/spec.md`.

## Phase 7 ⏳ — Universal per-type data export

Идея: единый export pipeline в Kepler shell, разные типы данных уходят в разные форматы:

- `note_obj` → markdown файлы (один файл = одна заметка)
- `time_entry_obj` → CSV / JSON
- `run_obj` (future, Olympia/Strava-likes) → GPX + zip с маршрутом и метаданными
- `game_obj` (Arrancador) → JSON библиотека + ссылки на assets
- `task_obj` → markdown с фронтматтером / CSV

Реализация — `kepler.export.<type>(filter, format)` API в shell preload + UI «Экспорт» в Settings, доступный из любого extension'а. Per-type конвертеры регистрируются как plugins в shell. Текущий Eden export-to-markdown заменяется этим механизмом.

## Phase 7.5 ⏳ — Adaptive lifecycle (optional)

Динамическое включение/выключение extensions на основе usage (LRU eviction, RAM budget). Зависит от Phase 4-6.

## Phase 8 ⏳ — Production packaging + retire legacy

### Packaging ✅

`bun run --cwd shell build` собирает финальный **NSIS one-click** установщик `release/Kepler Setup X.Y.Z.exe`. Конфиг — `shell/package.json → build`:

- `extraResources` копирует `kepler-backend.exe`, `ark-core-rpc.exe`, директорию `extensions/` (только `manifest.json`, `icon.png`, `index.html`, `dist/` — без `src/` / `node_modules/`), и `build/icon.png`.
- `afterPack` (`build/afterPack.cjs`) embed'ит иконку в `Kepler.exe` через `rcedit` + `png-to-ico` (стандартный паттерн Kosmos, см. [Horologion](./horologion.md), [Delphi](./delphi.md)).
- NSIS: `oneClick`, `perMachine: false` (install в `%LocalAppData%\Kepler` без UAC), `runAfterFinish: true`, desktop + Start Menu shortcuts, `deleteAppDataOnUninstall: false`.

Полная разбивка — [Kepler → Production packaging](./kepler.md#production-packaging-phase-8).

### Что осталось

- ⏳ **Auto-update** через `electron-updater`.
- ✅ Удаление старого Rust gpui launcher (выполнено в Phase A).
- Установщик переписывает HKCU Run на новый `Kepler.exe`.

## Phase 9 ⏳ — Delphi UI: Tailwind → plain CSS (открытый вопрос)

Delphi extension сейчас использует Tailwind v4 в templates (наследие legacy standalone Delphi). Все остальные extension'ы (Horologion, Dashboard, Arrancador, Kepler launcher / settings) написаны на **plain scoped CSS + `@kepler/visuals` CSS variables** — единый стиль через ecosystem.

Что нужно для Phase 9:

- Пройти ~30 .vue файлов в `extensions/delphi/src/{App.vue, components, pages}`.
- Удалить Tailwind utility classes (`flex items-center gap-2 px-3 rounded-lg ...`) из шаблонов.
- Переписать стили в `<style scoped>` с `var(--background) / --foreground / --border / --radius-*` из `@kepler/visuals`.
- Удалить `@import "tailwindcss"` + `@source` directive из `src/global.css`.
- Удалить `@tailwindcss/vite` plugin из `extensions/delphi/vite.config.mjs` + `vite.extensions.config.mjs`.
- Удалить `tailwindcss` + `@tailwindcss/vite` deps из `extensions/delphi/package.json` (и shared kepler-shell deps если нет других пользователей).
- Verify build + manual UI smoke (Delphi выглядит OK на acrylic Mica background).

Скоуп — несколько часов сфокусированной работы (или один agent). Откладываем до момента когда Delphi UI стабилизируется (project / area / settings flows не меняются часто) — иначе придётся переделывать дважды.

## Phase 10 ⏳ — Extension installer

Цель — отвязать жизненный цикл extensions от Kepler shell, чтобы итерация по конкретному приложению не требовала пересборки и переустановки launcher'а. Подробно — [Extension installer](/concepts/extension-installer).

### MVP ✅ (2026-05-14)

- **Resolution chain**: `extension-host.ts` поднимает user-installed копию из `%APPDATA%\Kosmos\extensions\<id>\` выше bundled `<resourcesPath>/extensions/<id>\`. Built-ins продолжают ехать с installer'ом как fallback.
- **CLI scripts** в `shell/scripts/`:
  - `bun run --cwd shell ext:install <path-to-extension-dir>` — копирует source в `<APPDATA>/Kosmos/extensions/<id>/` (atomic: tmp + rename + old backup).
  - `bun run --cwd shell ext:uninstall <id>` — удаляет user-папку. Bundled версия (если есть) поднимается автоматически.
- **listExtensions dedup** по `id` — первый встреченный root по приоритету выигрывает.

### Что осталось

- ✅ **Persistent extension user data** (2026-05-14) — split `extensions/<id>/` (код) vs `extensions-data/<id>/` (user data + window state); preload API `window.kepler.userData.*`; uninstall флаг `--purge-data`. См. [Extension installer](../concepts/extension-installer.md).
- ⏳ **`.kext` пакетный формат** — zip с manifest + dist + icon в одном файле. File association в Windows, открытие по двойному клику инсталлирует.
- ⏳ **UI manager в Kepler settings** — страница «Расширения»: список installed, кнопки install / remove / update.
- ⏳ **Auto-update** — checker для новых версий extension'ов (manifest version field + remote URL или local update file).
- ⏳ **Code signing / manifest validation** — verify publisher signature, schema validation, capability declarations (когда появятся third-party extensions).

## Баги / замечания

- ⚠️ **Win32 SetWindowPos jitter** при show/hide launcher окна — зафиксили: snap resize + GPU CSS Transition. Если регрессии в Phase 4 (extension windows) — смотреть туда же.
