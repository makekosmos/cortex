# Kepler — Roadmap

Pivot 2026-05-14: ecosystem `Kepler` → `Kosmos`, launcher `Kosmos` → `Kepler`. Все паттерны swap'нуты в коммите brand-swap. Эта страница — единый план работ по Electron host'у `platform/desktop/`.

## Статус по фазам

| Фаза | Что                                                                                                                                                                                                                       | Статус |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| 0    | Backend extracted в `platform/runtime/` (lib + bin `kepler-backend.exe`)                                                                                                                                                  | ✅     |
| 1    | Electron shell scaffold (launcher window, tray, settings, hotkey, backend spawn, window state)                                                                                                                            | ✅     |
| 2    | Command bus (Rust в backend + `@kosmos/ark` SDK + apps register + dynamic launcher)                                                                                                                                       | ✅     |
| 3    | Real handlers (Horologion / Delphi / Eden wired), Settings window, extension loader PoC                                                                                                                                   | ✅     |
| 4    | Apps как Vue extensions внутри Kepler (Dashboard / Horologion / Delphi / Arrancador)                                                                                                                                      | ✅     |
| 5    | Extension developer mode (Vite HMR per extension, Raycast-style)                                                                                                                                                          | ✅     |
| 6    | Eden как extension (Phase 6.0 scaffold + ARK note CRUD; Phase 6.0.A cleanup — Hevy/code-tools/standalone удалены, trash UI, codesplit)                                                                                    | ✅     |
| 7    | Universal per-type data export (notes → md, tasks → md/CSV, time entries → CSV, tags/games → JSON) — Rust converter framework + Settings UI                                                                               | ✅     |
| 7.5  | Adaptive lifecycle (optional)                                                                                                                                                                                             | ⏳     |
| 11   | Backup / Disaster Recovery ARK DB (multi-disk + GitHub + encryption)                                                                                                                                                      | ⏳     |
| 8    | Production packaging (NSIS) ✅ / auto-update ⏳ / retire legacy Rust gpui launcher ✅                                                                                                                                     | ⏳     |
| 9    | Delphi UI: Tailwind → plain CSS (открытый вопрос)                                                                                                                                                                         | ⏳     |
| 10   | Extension installer — CLI install/uninstall ✅ / `.kext` ⏳ / UI manager ⏳ / auto-update ⏳                                                                                                                              | ⏳     |
| 12   | Password manager (как хранить — TBD)                                                                                                                                                                                      | ⏳     |
| 13   | Raycast API совместимость — целевая в Kepler **0.5.0**; ready slices: manifest/no-view/view/menu-bar host, List/Form/Grid/Detail/MenuBarExtra, controlled List/Grid props, EmptyView actions, launchCommand, file actions | ⏳     |
| 14   | Eden state на **Pinia Colada** (auto-cache + dedup + revalidation, минус ~200 строк самопального state-sync)                                                                                                              | ⏳     |
| 16   | **AI semantic search** в ARK — sqlite-vec + fastembed-rs + DirectML на Windows, MiniLM bundle + Qwen3 opt-in                                                                                                              | ⏳     |

## Phase 0 ✅ — Backend extracted

Из старого Rust gpui launcher (легаси, удалён в Phase A) выделен чистый backend в `platform/runtime/`:

- Crate с lib (ARK runtime, WS server, command bus, sync) + бинарь `kepler-backend.exe`.
- Spawn'ится Electron host'ом как child process; держит ARK и обслуживает WS-клиентов.
- Сборка: `cargo build --release --manifest-path platform/runtime/Cargo.toml --bin kepler-backend`.

## Phase 1 ✅ — Electron shell scaffold

`platform/desktop/` — новая Electron-апка:

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
- **Eden**: `eden:note:create` (новая заметка), `eden:note:open-today` (открыть/создать сегодняшний journal entry). Поиск — внутри Eden (Ctrl+K в окне), отдельной команды `eden:search` нет.

Дополнительно:

- Settings window для Kepler shell (отдельный `BrowserWindow`, hash `#/settings`).
- Extension loader PoC: `electron/extension-host.ts` загружает static extensions из `extensions/<id>/{manifest.json,index.html,bundle.js}` в отдельные BrowserWindow'ы. Демо: `incubator/horologion/` и др.

## Phase 4 ✅ — Apps как Vue extensions

Цель достигнута: 4 апки рендерятся **внутри** Kepler как Vue extensions, без отдельных Electron-процессов. Eden мигрирован отдельно в Phase 6.0 / 6.0.A (см. ниже).

Мигрированы:

- **Dashboard** — полная Vue migration, build ~83 KB JS. Read-only аналитика, ARK через preload bridge. <span class="kbadge warn">после 2026-05-14 Dashboard rewritten — теперь встроенный shell view (ARK browser), не extension. См. [Dashboard](/apps/dashboard).</span>
- **Horologion** — полная Vue migration с `horologionApi` shim над `window.kepler.*`. Build ~102 KB chunk `pomodoroSettings`. Pomodoro/stopwatch state работает.
- **Delphi** — Vue + memory router, build 3483 modules. После Phase 5 cleanup: `electron-api-shim.ts` устанавливает `window.electronAPI` поверх `kepler.ark.request` — все existing call sites работают. Tailwind plugin подключён (Phase 5). **Открытый вопрос** — переписать Delphi UI с Tailwind utility classes на plain CSS + `@kosmos/visuals` tokens (как остальные extension'ы). См. Phase 9 ниже.
- **Arrancador** — изначально UI subset (LayoutPage + GameCard). После full completion (2026-05-18): scanner Steam+Epic, launcher (Steam URL + exe spawn), RAWG metadata, SQOBA save backups, все 4 страницы оживлены. См. [Arrancador](./arrancador.md).

RAM-эффект Phase 4 — −124 MB Working Set / −209 MB Private Bytes / −4 процесса. Полная таблица — [RAM benchmarks](/concepts/ram-benchmarks).

### Post-migration polish

После основной миграции Phase 4 добавлены доводки, считаются частью Phase 4:

- **App icons в launcher** — каждое open-command extension'а (Eden / Horologion / Delphi / Arrancador) показывает иконку из `extensions/<id>/icon.png`. `extensionIconDataUri(id)` в `extension-host.ts` читает PNG и кэширует по **mtime файла** — hot-swap иконки без рестарта Kepler. После Phase 6.0 Eden уже extension, иконка `products/eden/icon.png` участвует в общем механизме.
- **Crash on close fix** — `BrowserWindow.on("closed", …)` теперь использует captured `wcId` (захваченный **до** регистрации listener'а), а не `win.webContents.id` после destroy. До фикса Kepler падал при закрытии extension-окна.
- **Status dot в Horologion topbar** — точка статуса подключения к ARK (probe `list_object_types` каждые 10с), визуально совпадает с Delphi extension'ом.
- **Selected-space DB resolution** — `kepler-shell` main читает `%APPDATA%\Kosmos\selected-space.json` (через `@kosmos/ark` хелперы `readSharedSelectedSpace` / `getArkDbPathForSelectedSpace`) и передаёт `KOSMOS_DB_PATH=<spaceDir>/ark.db` в env при `spawnBackend()`. Backend пишет в выбранный space, а не в дефолтный `%APPDATA%\Kosmos\ark.db`.

## Phase 5 ✅ — Extension developer mode

Hot-reload для extensions через Vite dev servers, как `ray develop` у Raycast. Подробно — [Extension dev mode](/concepts/extension-dev-mode).

Состав:

- `bun run --cwd platform/desktop dev` поднимает shell dev session и Akasha HMR (`:5185`); `bun run --cwd platform/desktop dev:extensions` поднимает Vite dev server на отдельном порту для каждого Vue extension'а (5180–5185).
- Поле `devPort` в manifest + TCP probe в extension-host → живой порт резолвится в `loadURL('http://localhost:<port>/')`, мёртвый порт — в `loadFile(dist/...)`. `KEPLER_DEV=1` управляет shell-level dev, а не принудительным extension HMR.
- F12 toggles DevTools на любом extension window.

### Cleanup под Phase 5 (выполнено)

- **Delphi `electron-api-shim`** — `products/delphi/src/lib/electron-api-shim.ts` устанавливает `window.electronAPI` поверх `window.kepler.ark.request`, мапит legacy каналы (`ark:listDelphiTasks`, `ark:upsertDelphiTask`, `ark:deleteDelphiTask`, `ark:listTimeEntries`) на ARK operations. LAN sync / P2P / space management — graceful no-op. Подробно — [Delphi → electron-api shim](./delphi.md#electron-api-shim-в-extension).
- **Arrancador pages migration** — все 7 страниц мигрированы в extension (Library, Catalogue, Scan, Sqoba, Statistics, Settings, GameDetail). Native scanner / RAWG / game launch / usage heatmap пока stubs. Подробно — [Arrancador](./arrancador.md).
- **Tailwind restored для Delphi** — `@tailwindcss/vite` plugin подключён обратно в `products/delphi/vite.config.mjs`, т.к. оригинальный UI на Tailwind utility classes. Переписывание на plain CSS — открытый вопрос Phase 9.

## Phase 6 ✅ — Eden как extension

Eden — самый сложный кейс (TipTap editor + Heart Rust поиск + широкий preload API: titlebar history, store hardening, FTS, vault).

### Phase 6.0 (2026-05-17) — Scaffold + ARK note CRUD

- `products/eden/` создан как Vue extension: manifest (devPort 5184, 1100×750), package.json, vite config, src/ портирован из standalone Eden.
- `kepler-api-shim` (`products/eden/src/lib/kepler-api-shim.ts`) эмулирует `window.api` поверх `window.kepler.ark.request(...)`. Шаблон такой же, как Delphi `electron-api-shim` — позволяет сохранить ~50 call-sites Eden codebase'а без переписывания.
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

## Phase 7 ✅ — Universal per-type data export

Реализовано 2026-05-18. См. [Data export](/concepts/data-export) для полного описания механизма.

**Что есть:**

- Rust `Converter` trait + registry в `platform/runtime/src/export/`.
- 6 первых конвертеров: `note_md`, `task_md`, `task_csv`, `time_entry_csv`, `tag_json`, `game_json`.
- WS endpoints `export.list` / `export.run`.
- UI секция «Экспорт» в `platform/desktop/src/views/SettingsView.vue` с per-converter картой, native directory picker, история экспортов в localStorage.
- 17 unit tests для converters (cargo test 75/75 зелёный).

**Что НЕ в Phase 7** (отложено):

- Import обратно (collision rules + merge) — Phase 7.X если понадобится.
- Filter API по date range / tags — Phase 7.1.
- GPX для `run_obj` — нужен Olympia extension (отдельный proof loop).
- Encryption / scheduled export — Phase 11 backup перекрывает.

## Phase 7.5 ⏳ — Adaptive lifecycle (optional)

Динамическое включение/выключение extensions на основе usage (LRU eviction, RAM budget). Зависит от Phase 4-6.

## Phase 8 ⏳ — Production packaging + retire legacy

### Packaging ✅

`bun run --cwd platform/desktop build` собирает финальный **NSIS one-click** установщик `release/Kepler Setup X.Y.Z.exe`. Конфиг — `platform/desktop/package.json → build`:

- `extraResources` копирует `kepler-backend.exe`, `ark-core-rpc.exe`, директорию `extensions/` (только `manifest.json`, `icon.png`, `index.html`, `dist/` — без `src/` / `node_modules/`), и `build/icon.png`.
- `afterPack` (`build/afterPack.cjs`) embed'ит иконку в `Kepler.exe` через `rcedit` + `png-to-ico` (стандартный паттерн Kosmos, см. [Horologion](./horologion.md), [Delphi](./delphi.md)).
- NSIS: `oneClick`, `perMachine: false` (install в `%LocalAppData%\Kepler` без UAC), `runAfterFinish: true`, desktop + Start Menu shortcuts, `deleteAppDataOnUninstall: false`.

Полная разбивка — [Kepler → Production packaging](./kepler.md#production-packaging-phase-8).

### Что осталось

- ⏳ **Auto-update** через `electron-updater`.
- ✅ Удаление старого Rust gpui launcher (выполнено в Phase A).
- Установщик переписывает HKCU Run на новый `Kepler.exe`.

## Phase 9 ⏳ — Delphi UI: Tailwind → plain CSS (открытый вопрос)

Delphi extension сейчас использует Tailwind v4 в templates (наследие legacy standalone Delphi). Все остальные extension'ы (Horologion, Dashboard, Arrancador, Kepler launcher / settings) написаны на **plain scoped CSS + `@kosmos/visuals` CSS variables** — единый стиль через ecosystem.

Что нужно для Phase 9:

- Пройти ~30 .vue файлов в `products/delphi/src/{App.vue, components, pages}`.
- Удалить Tailwind utility classes (`flex items-center gap-2 px-3 rounded-lg ...`) из шаблонов.
- Переписать стили в `<style scoped>` с `var(--background) / --foreground / --border / --radius-*` из `@kosmos/visuals`.
- Удалить `@import "tailwindcss"` + `@source` directive из `src/global.css`.
- Удалить `@tailwindcss/vite` plugin из `products/delphi/vite.config.mjs` + `vite.extensions.config.mjs`.
- Удалить `tailwindcss` + `@tailwindcss/vite` deps из `products/delphi/package.json` (и shared kepler-shell deps если нет других пользователей).
- Verify build + manual UI smoke (Delphi выглядит OK на acrylic Mica background).

Скоуп — несколько часов сфокусированной работы (или один agent). Откладываем до момента когда Delphi UI стабилизируется (project / area / settings flows не меняются часто) — иначе придётся переделывать дважды.

## Phase 11 ⏳ — Backup / Disaster Recovery ARK DB

User-story: после смены ПК / переустановки Windows / умершего диска **данные ARK** (`%APPDATA%\Kosmos\ark.db`) восстанавливаются без потерь. Сейчас бэкапа нет, всё лежит на одном диске → 2026-05-17 user потерял часть истории сменой машины.

### Технический фон

ARK = SQLite в WAL-режиме. Просто `cp ark.db` рисково — WAL может содержать незакоммиченные транзакции. Нужен consistent snapshot:

- `VACUUM INTO 'dest.db'` — атомарно копирует БД в новый файл, без WAL и idle pages. Single-statement.
- Или `sqlite3_backup_init/step/finish` API — для online backup'а без блокировок.

Реализация — Rust в `kepler-backend`, потому что у него уже есть открытое соединение с ARK и оно знает корректный путь.

### Сценарий destinations

Backup destination — любой writable path. Это покрывает:

- **Multi-disk**: пользователь добавляет несколько локальных путей (`D:\backup\kosmos`, `E:\external\kosmos`).
- **Cloud sync folders**: Google Drive (`G:\My Drive\kosmos-backup`), Yandex.Disk (`Y:\kosmos-backup`), Dropbox, OneDrive — это всё локальные mount points для kepler-backend, не специальные API.
- **Network share**: SMB / NFS path работает прозрачно.

GitHub — отдельный destination type (не path). Auto-push в private repo через libgit2-rs или git CLI shell-out. ARK DB обычно <100MB → влезает без LFS.

### Фазы реализации

**Phase 11.0 — MVP**

- `platform/runtime/src/backup.rs`: функция `snapshot_to(dest: PathBuf) → Result` через `VACUUM INTO`.
- WS endpoint `backup.snapshot {dest}` для shell.
- UI: `BackupsSettings.vue` в Kepler shell — кнопка «Создать бекап сейчас» + path picker.
- Storage: `%APPDATA%\Kosmos\backup-config.json` хранит список destinations + history `[{dest, started_at, finished_at, size_bytes, ok, error}]`.
- Manual только, без scheduler'а.

**Phase 11.1 — Multi-destination + scheduler**

- Несколько destinations одновременно в config'е.
- Scheduler в kepler-backend (раз в N часов / при shutdown).
- Auto-rotation: хранить N последних backup'ов на каждом destination, удалять старые.
- Если один destination недоступен (диск отключён) — продолжать с остальными, не падать.

**Phase 11.2 — GitHub destination**

- Destination type `github`: PAT + repo + branch.
- Push: compress (`ark.db.zst`), git commit, force-push в branch `kosmos-backup`.
- Хранить PAT encrypted в OS keychain (`keyring-rs`).

**Phase 11.3 — Restore UI + integrity verification**

- «Восстановить из бекапа»: выбрать destination → проверить integrity (SQLite `PRAGMA integrity_check`) → atomic replace `ark.db` (через backend shutdown + swap + restart).
- При плановых backup'ах — verify integrity на свежем snapshot'е.

**Phase 11.4 — Encryption at rest**

- Cloud destinations (особенно GitHub) хранят user-personal data → должны быть encrypted.
- `age` (через `age-encryption.rs`) — простой формат, по passphrase или X25519 key.
- Восстановление требует passphrase / key file.

### Открытые вопросы

- **Версионирование**: snapshot replaces или append-history (git commits каждого snapshot'а — full history)? MVP — replace, V3 — добавить retain-N-versions.
- **Where to schedule**: kepler-backend (Rust cron) или kepler-shell main process (Node timer)? Backend честнее — работает даже если shell закрыт; но требует kepler-backend running как service / autostart.
- **UI размещение**: Settings → Backups, или отдельный extension `backup-manager`? Скорее всего Settings → Backups (это core data resilience, не extension'ская функция).

### Связано

- [Граница записи](../concepts/write-boundary.md) — backend держит exclusive write handle, поэтому он же делает snapshot.
- [Изоляция тестовых БД](../concepts/test-isolation.md) — тесты для backup используют свой `KOSMOS_DATA_DIR`, не реальный.

## Phase 10 ⏳ — Extension installer

Цель — отвязать жизненный цикл extensions от Kepler shell, чтобы итерация по конкретному приложению не требовала пересборки и переустановки launcher'а. Подробно — [Extension installer](/concepts/extension-installer).

### MVP ✅ (2026-05-14)

- **Resolution chain**: `extension-host.ts` поднимает user-installed копию из `%APPDATA%\Kosmos\extensions\<id>\` выше bundled `<resourcesPath>/extensions/<id>\`. Built-ins продолжают ехать с installer'ом как fallback.
- **CLI scripts** в `platform/desktop/scripts/`:
  - `bun run --cwd platform/desktop ext:install <path-to-extension-dir>` — копирует source в `<APPDATA>/Kosmos/extensions/<id>/` (atomic: tmp + rename + old backup).
  - `bun run --cwd platform/desktop ext:uninstall <id>` — удаляет user-папку. Bundled версия (если есть) поднимается автоматически.
- **listExtensions dedup** по `id` — первый встреченный root по приоритету выигрывает.

### Что осталось

- ✅ **Persistent extension user data** (2026-05-14) — split `extensions/<id>/` (код) vs `extensions-data/<id>/` (user data + window state); preload API `window.kepler.userData.*`; uninstall флаг `--purge-data`. См. [Extension installer](../concepts/extension-installer.md).
- ⏳ **`.kext` пакетный формат** — zip с manifest + dist + icon в одном файле. File association в Windows, открытие по двойному клику инсталлирует.
- ⏳ **UI manager в Kepler settings** — страница «Расширения»: список installed, кнопки install / remove / update.
- ⏳ **Auto-update** — checker для новых версий extension'ов (manifest version field + remote URL или local update file).
- ⏳ **Code signing / manifest validation** — verify publisher signature, schema validation, capability declarations (когда появятся third-party extensions).

## Phase 12 ⏳ — Password manager

::: warning Открытый вопрос
Целевое поведение и storage backend пока **не определены**. Идея — добавить безопасное хранилище секретов (пароли / API-ключи / personal access tokens), доступное из launcher'а и extension'ов через типизированный API.
:::

### Что обсуждается

- **Storage**: OS keychain (Windows Credential Manager / Keyring) через `keyring-rs` vs encrypted SQLite в ARK (с master key через `age` или passphrase) vs внешняя интеграция (Bitwarden CLI / 1Password CLI).
- **API surface**: команды launcher'а (`password:search`, `password:copy`) + extension API (`window.kepler.secrets.{get,set,list}`) с per-extension permission scoping.
- **Auto-fill**: целевой scope первой версии — copy-to-clipboard с auto-clear через 30s. Auto-fill через accessibility API — Phase 12.1+.
- **Sync**: если SQLite-based — едет по существующему ARK sync. Если keychain-based — per-machine, без sync.
- **TOTP**: в первой версии не обязательно, можно после MVP.

### Известные риски

- Хранение чувствительных данных требует тщательной thread-модели (где master key, кто может его прочитать, что попадает в логи / crash dumps / backup'ы).
- Если SQLite-based — backup-feature из Phase 11 должна понимать «pillaring» (некоторые таблицы не шифруются, password vault — шифруется отдельно).

Решение — отдельным spec'ом перед началом реализации.

## Phase 14 ⏳ — Eden state на Pinia Colada

Цель — заменить **самопальный** server-state-management в `products/eden/src/store/eden.ts` (saveCoordinator + latestSaveTimestamps + manual `entries.value[idx]` обновления) на queries/mutations [Pinia Colada](https://pinia-colada.esm.dev/) (v1.3.0+, аналог TanStack Query для Vue).

### Что даст

- **Минус ~200 строк** custom state-sync кода (саму идею «когда entry обновился, найти его в entries.value по id и заменить» можно перестать писать руками).
- **Auto dedup** для конкурентных save (сейчас руками через `saveCoordinator.inFlight/queued`).
- **Stale-while-revalidate** для `listEntries` (показываем кэш пока ARK refetch'ится).
- **Background refetch на window focus** — entries автоматически обновляются когда юзер возвращается к Eden.
- **Optimistic updates** для mutations с auto-rollback при ошибке.
- **Лишает класса багов** типа [silent-save-skip](https://github.com/ksanrse/kepler/commit/acd31732) — мы перестаём писать stale-detection логику сами, делегируем библиотеке.

### План

Pinia Colada **коэкзистит** с обычной Pinia → миграция инкрементальная, не big-bang:

1. **Pilot** (~30 мин): `bun add @pinia/colada` в `products/eden/`, `app.use(PiniaColada)` в `main.ts`. Установка — drop-in, никакого behavior change. Этот шаг можно сделать сейчас (E3 в [experiments harness](/agents/spec-templates#experiments)).
2. **Query #1**: конвертировать `listEntries` → `useQuery({ key: ['entries'], query: listEntries })`. Удалить ручной `entries.value = await listEntries()` из `initApp` / `openTodayJournal` / `refreshData`.
3. **Query #2**: `loadEntry(id)` → `useQuery({ key: ['entry', id], query: () => loadEntry(id) })`.
4. **Mutation #1**: `saveEntry` → `useMutation({ mutation: saveEntry, onSuccess: () => queryCache.invalidate(['entries']) })`. Удалить `saveCoordinator` и `latestSaveTimestamps`.
5. **Mutation #2**: `deleteEntry`, `saveNoteType` — аналогично.
6. **Cleanup**: удалить orphan reactive refs (`entries.value` если больше никем не используется).

### Метрики (proof loop обязателен)

| Метрика                                 | До     | После | Цель                              |
| --------------------------------------- | ------ | ----- | --------------------------------- |
| `wc -l products/eden/src/store/eden.ts` | 686    | TBD   | ≤ 500                             |
| Eden bundle `index.js` gzip             | 113 KB | TBD   | ≤ 125 KB (стоимость либы ~5-10KB) |
| `tests/e2e/eden.spec.ts` wall-clock     | TBD    | TBD   | не выросло                        |

baseline снят в `.agent/experiments/2026-05-19-tooling-pass/` (E3).

### Riски

- Pinia Colada query-key invalidation — нужно правильно подобрать гранулярность ключей. Слишком грубо → лишние refetch'и; слишком тонко → stale data.
- `useQuery` / `useMutation` — composables, должны вызываться в `setup` Vue компонента. Store-функции `eden.ts` (вне компонента) не могут их звать напрямую — нужен рефакторинг API store'а (либо store возвращает queries/mutations refs, либо переезжаем на queries-from-composables паттерн).

## Phase 13 ⏳ — Raycast API совместимость <Badge type="tip" text="target: 0.5.0" />

Цель — поддержать **подмножество [Raycast Extension API](https://developers.raycast.com/api-reference)** так, чтобы существующие Raycast extensions могли быть портированы в Kepler с минимальными правками (или вообще без — через адаптер-loader). К релизу Kepler **0.5.0** — обязательно.

### Что планируется поддержать (минимальный паритет)

- **Commands**: `view` / `no-view` / `menu-bar` modes из Raycast `package.json`.
  Phase 13 foundation уже мапит `no-view` в `raycast-no-view`, а trusted
  `view` commands — в serializable `List` host. `menu-bar` commands уже
  мапятся в `raycast-menu-bar` и рендерятся через первый `MenuBarExtra` host.
  Отсутствующий `mode` default'ится в `view`, явные неизвестные modes
  отбрасываются на этапе manifest parsing.
- **List / Detail / Form / Grid компоненты** — host-controlled command primitives,
  а не обычные Vue widgets в `@kosmos/visuals`: `ActionPanel` должен знать
  selected item, shortcuts, navigation, clipboard, close behavior и command lifecycle.
  Сейчас реализован первый shell host для `List`, root/inline `Detail`
  markdown, `List.Item.Detail` aliases, root `Detail.actions`, `Detail.Metadata`, `ActionPanel`,
  `ActionPanel.Section`, `ActionPanel.Submenu`, `Keyboard.Shortcut.Common`
  и action `shortcut` labels/key dispatch snapshot'ов: работают
  `Action.CopyToClipboard`,
  `Action.OpenInBrowser`, `Action.Open`, `Action.LaunchCommand`,
  `Action.Pop`, `Action.PopToRoot` и локальный
  `Action.Push` в detail target; generic `Action` callbacks, `showToast`,
  `showHUD` и `confirmAlert` подключены к host feedback/confirm bridge;
  `List.Section`, `List.EmptyView` with footer actions,
  `List.Item.icon`, `List.Item.accessories`, `List.Dropdown` search accessory и controlled `List.searchText` /
  `List.selectedItemId` / `List.filtering` с `onSearchTextChange` /
  `onSelectionChange` уже учитываются. Также есть первый `Form`
  host (`TextField`, `PasswordField`, `TextArea`, `Checkbox`, `Dropdown`,
  `Dropdown.Section`, `Description`, `Separator`, `TagPicker`, `DatePicker` Date defaults/onChange, `FilePicker`) с `Action.SubmitForm` callback'ом и guarded
  field `onChange` callback'ами, Form footer common actions, плюс guarded native file dialog для trusted commands. Первый
  `Grid` host уже рендерит
  `Grid.Section`, `Grid.Item`, `Grid.EmptyView` with footer actions, image/placeholder previews,
  search, `Grid.Dropdown` search accessory, controlled `Grid.searchText` /
  `Grid.selectedItemId` / `Grid.filtering` callbacks, `isLoading` и
  selected-item `ActionPanel`; `List.isLoading` тоже отображается. Первый
  `MenuBarExtra` host рендерит
  `MenuBarExtra.Section`, `MenuBarExtra.Item`, `MenuBarExtra.Submenu` и item
  callbacks через guarded session IPC. `useNavigation().push/pop/popToRoot`
  ведёт session stack в Electron host и обновляет root snapshot renderer'а.
  Rich navigation polish остаётся дальше.
- **`@raycast/api`**: private workspace shim `packages/raycast-api` (`@raycast/api`)
  уже содержит начальные `Clipboard.copy/read/readText/clear`, `showToast`, `getPreferenceValues`,
  `LocalStorage` включая `allItems`, `Cache`, `launchCommand`, `List`, `Detail`, `ActionPanel`,
  `Action` и bridge для `@raycast/api/jsx-runtime`. `launchCommand` уже ходит через shell declared-command registry для
  trusted `view` / `no-view` / `open` targets.
- **Preferences UI** — Raycast extensions объявляют preferences в manifest, Kepler рендерит автоматически в Settings → Расширения → <Имя>.
- **Keyboard shortcuts** — первый renderer-level action `shortcut` dispatch уже есть; дальше нужен command-bus-level mapping для глобальных/launcher shortcuts.

### Что НЕ войдёт в 0.5.0

- AI commands API (Raycast Pro feature) — отложить.
- Quicklinks / Snippets — overlap с существующими Kepler-командами, обсуждаемо.
- Cloud sync настроек Raycast — n/a, у нас свой sync.

### Открытые вопросы

- **Loader**: foundation использует private `@raycast/api` workspace package и
  Raycast `package.json` parser. Открыто: command bundler / import aliasing для
  real TS/TSX Raycast projects, включая JSX runtime entrypoints.
- **Лицензия**: Raycast API типы (`@raycast/api`) — proprietary. Использовать
  TypeScript types из их пакета нельзя; совместимый shape объявляется своими
  силами в `packages/raycast-api`.
- **Marketplace**: установка Raycast extensions через `.kext` (после конвертации) vs прямая поддержка `.raycast` бандлов.

## Phase 16 ⏳ — AI semantic search

::: tip Материализация AI-first графа
North-star проекта — «всё есть объект + AI-friendly граф». Phase 16 — первая конкретная фича в этом направлении: semantic search через embeddings, локально, без сетевых API.
:::

### Цель

Дать пользователю поиск по смыслу, не только по точному тексту. «Найди заметки про prosemirror save flow» должно работать даже если в заметке слово «prosemirror» вообще не упомянуто — embedding отражает смысл, не лексику.

### Стек

| Компонент                                  | Что                                                                                                                               | Почему                                                                                          |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| **sqlite-vec** (extension)                 | KNN-поиск по векторам в той же SQLite-БД где живёт ARK. Brute-force в MVP, ANN-индексы (DiskANN/IVF) позже когда стабилизируются. | One DB to rule them all. Sync через ARK работает «бесплатно».                                   |
| **fastembed-rs**                           | Inference embeddings локально через ONNX Runtime. Multi-platform, без сети.                                                       | Pure Rust, без Python deps. Уже на 5.x stable.                                                  |
| **DirectML execution provider** (Windows)  | GPU-инференс embedding-моделей через DirectX 12. Любой GPU (Nvidia/AMD/Intel/Qualcomm).                                           | 5-10× быстрее CPU для bulk-reindex (тысячи заметок при первой раскрутке).                       |
| **MiniLM-L6-v2** (default)                 | 22M params, 384-dim, ~30MB q4 на диске. Bundle с installer.                                                                       | MVP: маленькая модель, посредственно на русском но работает.                                    |
| **Qwen3-Embedding-0.6B** (opt-in download) | 600M params, 1024-dim, ~300MB q4. **On-demand**, не bundled.                                                                      | Quality upgrade для RU/EN смешанных заметок. Settings → «Включить умный поиск с RU-поддержкой». |

### Фазы

1. **Phase 16.0 MVP** — sqlite-vec extension load, MiniLM bundled, brute-force KNN, индексация при `upsert_object`. Search-API `semantic_search_objects(query, limit)`. Eden UI: чекбокс «semantic» в SearchOverlay.
2. **Phase 16.1** — DirectML на Windows (только Windows + GPU fallback на CPU). Bulk reindex для existing entries (фоновая задача, прогресс-бар в Dashboard).
3. **Phase 16.2** — Qwen3 как opt-in download в Settings → Фокус → Smart Search. Conversion script для re-index'а от MiniLM → Qwen3 (если юзер переключается).
4. **Phase 16.3** — ANN-индексы (когда sqlite-vec DiskANN/IVF стабилизируется из alpha). До 10k заметок brute-force OK, дальше нужен индекс.

### Открытые вопросы

- **Языковая стратегия**: одна модель на все языки (Qwen3 — да) vs separate по языку (MiniLM EN + RU-специфичная)?
- **Privacy**: embeddings содержат смысл текста заметок. Хранятся в той же ARK DB, рискуют попасть в backup'ы. Encrypt-at-rest для embeddings — нужен или нет?
- **Sync**: 1024-dim Qwen3 embeddings — это +4KB на заметку. Для 10k заметок — +40MB к ARK DB. По LAN sync передаётся всё. Acceptable?
- **Hybrid search**: combining FTS5 (lexical) + sqlite-vec (semantic) с rerank'ом — стандартный паттерн (Reciprocal Rank Fusion). В MVP или позже?
- **Eden vs другие apps**: search-API универсальный (любой object), но UI сначала в Eden. Когда подключить Delphi (поиск задач) / Horologion (поиск time entries)?

### Connected memories

- [[project-north-star-object-graph]] — обоснование почему вообще делаем.
- [Roadmap Phase 7 (Universal export)](#phase-7--universal-per-type-data-export) — обратная сторона: данные читаемы для людей в md/csv. Phase 16 — данные читаемы для AI через embeddings/MCP.

## Баги / замечания

- ⚠️ **Extension window drag rollback under system load** — 2026-06-08 debug: на Eden/Delphi
  при drag окна под высокой системной нагрузкой (например после Valorant) Win32 `GetWindowRect`
  иногда фиксирует реальный откат bounds примерно через 100ms после движения (`Delphi` дошёл до
  `x=1482,y=193`, затем тем же hwnd/size откатился на `x=1371,y=197`). Это не Eden-specific:
  общий слой `platform/desktop/electron/extension-host.ts` создаёт extension windows как
  `frame: true` + `titleBarStyle: "hidden"` + `titleBarOverlay` + transparent
  `backgroundMaterial: "acrylic"`, а draggable зона идёт через CSS `-webkit-app-region: drag`
  в `packages/visuals/components/Titlebar.vue`. Гипотеза: редкий Win32/DWM/Electron chrome-path
  становится чувствителен к GPU/CPU pressure; Discord не показатель, потому что может использовать
  другой titlebar/backdrop strategy. Минимальные A/B проверки: (1) временно `windowEffect: "none"`
  для Eden/Delphi; (2) оставить flat background, но изолировать `titleBarOverlay`; (3) отдельно
  убрать startup spikes (`file_index` lazy/idle, `usage_tracker` adaptive backoff), потому что
  нагрузка выглядит усилителем, а не единственной причиной.
- ⚠️ **Win32 SetWindowPos jitter** при show/hide launcher окна — зафиксили: snap resize + GPU CSS Transition. Если регрессии в Phase 4 (extension windows) — смотреть туда же.
