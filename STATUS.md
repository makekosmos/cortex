# Kosmos — статус проекта (2026-05-20)

## Текущие версии

| Артефакт | Версия |
|---|---|
| Kepler shell (`shell/package.json`) | **0.2.1** |
| Eden extension (`extensions/eden/manifest.json`) | **0.1.8** (Pattern B + Anytype-style block selection + Linear statuses) |
| Delphi extension | **0.1.3** (live ARK sync + «Когда-нибудь» tab) |
| Horologion extension | **0.1.5** |
| Arrancador extension | **0.1.3** |
| Dashboard | встроен в shell (не extension) |

## 2026-05-20 — Eden Pattern B + Anytype block selection (Kepler 0.2.0 → 0.2.1)

Большая итерация по Eden и cross-extension инфраструктуре.

### Eden 0.1.0 → 0.1.8: TipTap TaskList → Pattern B (task = object)

- **TipTap TaskList → TaskRef NodeView** (Pattern B refactor): task в заметке = ссылка
  на `task_obj` в ARK, не дубликат текста. Vue NodeView подписан на ARK
  `object_upserted`/`object_deleted` events, live обновляется когда Delphi меняет
  task. Title редактируется inline в input. Single click — открыть, Backspace
  на пустом — soft-delete.
- **Linear-style 5 статусов задач**: triage (default для новых) / backlog / todo /
  done / canceled. ПКМ-меню для выбора. Custom SVG icons на TaskStatusIcon.vue.
- **Anytype-style rubber-band block selection**: composable `useBlockSelection`,
  document-level mouse handlers с 20px threshold + block-crossing activation
  (drag в одной строке = native text select, drag через блоки = rubber-band).
  Visual через PM Decoration API (не direct DOM mutation — стирается на PM
  re-render). Esc clears, Delete удаляет блоки (для taskRef + soft-delete task_obj).
- **`/задача` и `- [ ]` markdown shortcut** создают TaskRef.
- **Persist state**: zen mode + last visited entry в localStorage. Reload
  возвращает в ту же заметку в том же режиме.
- **Char counter в zen mode**: IBM Plex Mono, fully transparent footer +
  border-top при scroll, padding-bottom редактору чтобы text упирался выше.
- **Brand orange каретка** для всех inputs/contenteditable.
- **CSS dedupe**: убраны 4 копии `.ProseMirror > * + *` правила, merge resize-handle
  3 копий, reset `<p>` margin. Unified 2px inter-block gap.

### Foundation (ark-core + shell)

- **`ark-core` local `entity_changed` events**: до 2026-05-20 эмитились только
  на sync-incoming изменения; теперь и на локальные `upsert_object` /
  `delete_object`. Cross-app live reactivity (Eden TaskRef ↔ Delphi list) теперь
  работает без specific protocols.
- **`ws_server` forward** ark-core events клиентам — был latent gap,
  forward'ились только command_bus + pomodoro events.
- **Shell: focus existing extension window** на повторный invoke. Включая
  minimized/hidden cases через AOT-toggle workaround для Win32
  `SetForegroundWindow` restriction. Применимо ко всем extension'ам.

### Delphi 0.1.1 → 0.1.3

- **Live sync через ARK events**: подписка на `object_upserted` для `task_obj`
  → refresh task list. Eden создаёт/меняет задачу — Delphi мгновенно видит.
- **Вкладка «Когда-нибудь»** в sidebar — задачи с `propsJson.status === "backlog"`
  (плюс legacy `isSomeday`). Иконка Archive.

### Visuals

- Новый primitive `Checkbox` — outline + inner filled square (Delphi
  `.check-box` parity). Brand accent через CSS var override (Eden — orange).

### Tooling

- Drive-by fix `scripts/ark-smoke.mjs` — Windows quote bug при `shell:true` +
  пробелы в path к `node.exe`.

## 2026-05-18 — performance sweep + UX полировка (Kepler 0.1.10 → 0.1.16)

Серия release'ов за день:
- **0.1.10** — Mica backdrop + electronLanguages shrink (−45 MB disk) + CSS contain + TS incremental
- **0.1.11** — Export bug fix (unwrap `{converters: [...]}`) + extensions catalog populated
- **0.1.12** — Marketplace UI (Settings → Расширения → catalog с кнопкой Установить) + Export tab скрыт (техдолг)
- **0.1.13** — Launcher commands filtered by installed extensions (Eden / Delphi / etc не показываются если не установлен)
- **0.1.14** — Toggle цвет = акцент (был зелёный) + streamer mode (Chromium occlusion off, shell-wide)
- **0.1.15** — **Focus widget** — Spotify-mini-player-style плавающий always-on-top окно для активной pomodoro
- **0.1.16** — Horologion task input alignment fix + QuickEntryPanel removed Tailwind + Storybook 8 + handcrafted convention + Focus mode roadmap spec

См.:
- `docs-site/concepts/system-requirements.md` — что нужно для запуска и сборки (canonical).
- `docs-site/concepts/performance-experiments.md` — реальные baseline/after measurements (правило: нет цифр → `(не записал)`, **никогда** не выдумывать).
- `.agent/tasks/2026-05-18-*/evidence.md` — proof loops с calibration.
- `packages/visuals/STORYBOOK.md` — Storybook contributor guide.
- `.agent/tasks/2026-05-18-focus-mode-digital-cave-spec/spec.md` — roadmap для digital-cave merger.

## 🛡 Production hardening (2026-05-18)

Подготовлен safety net для distributed local-first продукта с real users.
Proof loop: `.agent/tasks/2026-05-18-pre-in-process-hardening/`. Детали:
[docs-site/concepts/db-resilience.md](./docs-site/concepts/db-resilience.md).

- **DB backup** на старте backend — раз в 24h, rotation 7. `<data_dir>/backups/`.
- **`PRAGMA integrity_check`** в `init_schema` — fail loud на corruption.
- **Backend auto-respawn supervisor** в Electron main — exponential backoff (1s→5s→30s→1m→2m), error dialog после 5 streak'ов.
- **Crash reporter** — Rust panic_hook → `<data_dir>/crashes/panic-*.log` + Electron `crashReporter.start()`. Settings UI секция "Отчёты об ошибках".
- **Mutex poison recovery** — SqliteStorageBackend методы.
- **Property-based tests** — `cargo test --test proptest_invariants` (3 properties × 64 cases).

In-process facade (subprocess removal) — отложено: subprocess нужен пока как crash isolation layer. Hardening это **prerequisite**, не subprocess removal сам.

## 🟡 Хранимый техдолг (2026-05-18)

- **Export tab** скрыт в Settings (whitescreen на production 0.1.11). См. `docs-site/agents/manual-tests-pending.md` → tech debt entry. Возврат после DevTools debug.
- **QuickEntryPanel** в @kosmos/visuals — **handcrafted: false** (agent-built temp). Стилистика на токенах, но детали UX к доработке (badge показывает в Storybook).
- **`vue-router` mock в Storybook preview** — Sidebar/SidebarButton stories skipped (зависят от RouterLink).
- **Light theme** — TODO в `packages/visuals/.storybook/preview.ts` (theme toolbar item закомментирован).

Итог архитектурного pivot'а от standalone Electron-апок к Kepler-host архитектуре с Vue extensions. **Все 5 апок мигрированы** (Eden — Phase 6.0 + 6.0.A, 2026-05-17). После 2026-05-15: концепция spaces убрана (single DB per user), Dashboard встроен в shell, e2e Playwright suite зелёный. После 2026-05-17 (Phase 6.0.A): standalone `apps/eden/ts/` удалён, Hevy/code-tools убраны из Eden (Hevy → Olympia позже).

## Архитектура

```
Kepler.exe (Electron host)
  ├─ launcher window (Ctrl+Shift+K, fixed 720×460, acrylic)
  ├─ settings window (tray menu)
  ├─ Dashboard window (embedded shell view — read-only ARK browser)
  ├─ extension-host
  │   ├─ extensions/horologion/  (Vue bundle inside Kepler)
  │   ├─ extensions/delphi/      (Vue bundle inside Kepler)
  │   └─ extensions/arrancador/  (Vue bundle inside Kepler)
  └─ spawn kepler-backend.exe (Rust, headless)
      ├─ ark-core-rpc child (SQLite WAL, FTS5)
      ├─ WS server 127.0.0.1:<port>
      ├─ command bus (registry + invoke broadcast)
      └─ LAN sync (centralized — один node на машину)

Eden.exe — остаётся standalone Electron + общий backend через kepler-mode (legacy).
```

## Naming convention (после brand swap)

| Слой | Имя |
|---|---|
| Ecosystem (monorepo, ARK SDK, AppData) | **Kosmos** |
| Launcher app (Electron host) | **Kepler** |
| TS package SDK | `@kosmos/ark` |
| Visuals (CSS tokens + Vue components) | `@kosmos/visuals` |
| Backend binary | `kepler-backend.exe` |

## Изоляция data dir (3 уровня)

| Когда | Где live ark.db |
|---|---|
| **Production install** (NSIS) | `%APPDATA%\Kosmos\` |
| **Dev** (`bun run --cwd shell dev`) | `%APPDATA%\Kosmos-dev\` — отдельная от prod |
| **Test** (Playwright e2e) | `tests/.e2e/<slug>/` — fresh per spec |

Resolution в `shell/electron/data-dir.ts` `keplerDataDir()`:
1. `KOSMOS_DATA_DIR` env (test) → absolute path
2. `VITE_DEV_SERVER_URL` set (dev) → `Kosmos-dev`
3. иначе (prod) → `Kosmos`

Backend получает `KOSMOS_DATA_DIR=<resolved>` env при spawn'е. См. `docs-site/concepts/test-isolation.md` и `docs-site/agents/forbidden.md`.

## ✅ Сделано

### Phase 0 — Backend extraction

- `services/kepler-backend/` (lib + bin) — Rust headless service. Extracted из старого `apps/kosmos/` (legacy Rust launcher).
- 41 → 46 unit tests passing (5 новых для command_bus).

### Brand swap (Kepler ↔ Kosmos)

- 836 файлов pre-swap → post-swap. Старые apps `Kepler` → ecosystem `Kosmos`. Старый launcher `Kosmos` → новый `Kepler`.
- User data migration script: `scripts/migrate-kepler-to-kosmos.ps1` + smoke test (16 assertions pass).
- ADR: `docs/MIGRATION-2026-05-14-brand-swap.md`.

### Phase 1 — Electron Kepler shell

- `apps/kepler-shell/` — Electron 41 + Vue 3.6 + electron-vite + TypeScript.
- Frameless launcher 720×460, acrylic Mica на Win11, globalShortcut Ctrl+Shift+K.
- Tray icon + menu (Открыть / Настройки / Выход).
- Spawn `kepler-backend.exe` child + auto-connect через `ensureKeplerRunning`.
- Window state persistence в `%APPDATA%\Kosmos\kepler-shell-window-state.json`.
- DevTools auto-open detached в dev mode.

### Phase 2 — Command bus full stack

- `services/kepler-backend/src/command_bus.rs` — Rust WS-протокол.
  - Operations: `commands.register / unregister / list / invoke`.
  - Events: `command_invoked` / `commands_changed` broadcast.
  - Auto-unregister на WS disconnect.
- `@kosmos/ark` SDK: `client.commands.{register,unregister,list,invoke,onInvoked,onChanged}` namespace + types (`CommandManifest`, `CommandInvokedEvent`).
- Apps (Horologion / Delphi / Eden) регистрируют свои «ручки» при подключении в kepler-mode.

### Phase 3 — Real action handlers

- **Horologion**: `pomodoro:25 / 50 / stopwatch:start` действительно стартуют таймеры через usePomodoro composable + `workMinOverride` (без мутации persistent settings).
- **Delphi**: `task:create` → QuickEntry, `task:today` → router.push '/today'.
- **Eden**: `note:create` → `createNewEntry`, `note:search` → `openSearch`.
- **Kepler settings window** — separate BrowserWindow с автозапуском HKCU toggle + backend status + version.
- **Extension loader PoC** — foundation для Phase 4.

### Phase 4 — Apps как Vue extensions

| App | Build | Что работает | Что осталось |
|---|---|---|---|
| **Dashboard** | 83 KB JS / 17 KB CSS | Read-only analytics: Overview + Sessions pages через `kepler.ark.request("get_usage_analytics")` + focus/visibility auto-refresh | Choose/reset DB — host-managed; vue-router history dropped |
| **Horologion** | 102 KB pomodoroSettings chunk + 23 KB HomeView + 5 KB SettingsView | Pomodoro + Stopwatch + Settings через `horologionApi` shim над `kepler.ark.*`. Status dot + settings button в topbar. | Streamer mode / tray / multi-window settings — dropped |
| **Delphi** | 3485 modules, 32 KB CSS с Tailwind | Vue Router (memory history), 5 pages, `electron-api-shim.ts` устанавливает `window.electronAPI` поверх `kepler.ark.request`. Task CRUD работает | LAN sync / P2P / space management — graceful no-op. Tailwind остался (см. Phase 9) |
| **Arrancador** | 108 KB JS / 19 KB CSS, 57 modules | 7 страниц (Library + 6 stubs/read-only): Library, Catalogue (stub), Scan, Sqoba (stub), Statistics (JS aggregation), Settings (localStorage), GameDetail | Native scanner spawn, game launch, RAWG metadata fetch — Phase 5+ |

### Phase 5 — Extension developer mode (Raycast-style)

- Vite dev server per extension с unique портом (5180-5183 для dashboard/horologion/delphi/arrancador).
- `KEPLER_DEV=1` env var → extensions грузятся с `http://localhost:<port>/` вместо `dist/index.html`. HMR работает.
- F12 toggles DevTools на любом extension window.
- Settings → Developer Mode toggle (persist в `kepler-shell-settings.json`).
- `bun run dev:extensions` orchestrator + `bun run dev:kepler` paired workflow.

### Phase 8 — Production packaging

- `electron-builder` NSIS config: `apps/kepler-shell/package.json` `build` section.
- `extraResources`: `kepler-backend.exe` + `ark-core-rpc.exe` + `extensions/<id>/dist+manifest+icon` + tray icon.
- `afterPack.cjs` hook — PNG → ICO + rcedit embed metadata в `Kepler.exe`.
- `bun run --cwd apps/kepler-shell build` → `release/Kepler Setup 0.1.6.exe`.
- Install path: `%LOCALAPPDATA%\Programs\Kepler\` (per-user oneClick).

### Inter-app communication

- `@kosmos/ark` cosmos-mode — apps подключаются к single `kepler-backend` WS, делят `ark-core-rpc`. Один sync node на машину (-3 ark-core-rpc).
- Command bus: launcher → backend → apps event broadcast → handler execute.
- Selected space DB resolution — `kepler-shell` передаёт `KOSMOS_DB_PATH` в backend (читает `selected-space.json`).

### RAM benchmark

| Метрика | Baseline (4 standalone) | Kepler + 4 extensions | Diff |
|---|---|---|---|
| Working Set | 1092 MB | 968 MB | **−124 MB / −11%** |
| Private Bytes | 683 MB | 474 MB | **−209 MB / −31%** |
| Processes | 15 | 11 | −4 |

Caveats: оба measurements в dev mode (DevTools overhead +~160 MB). Real prod без DevTools — save аналогичный.

### Documentation

- `docs-site/concepts/`: architecture, command-bus, extension-host, extension-dev-mode, ram-benchmarks, sync, ark-objects, write-boundary, test-isolation.
- `docs-site/apps/`: kepler, kepler-roadmap, horologion, delphi, dashboard, arrancador.
- `docs-site/packages/`: kosmos-ark (с Commands API), kosmos-visuals, ark-core.
- `docs-site/reference/`: commands, rules, decisions, smoke-matrix.
- `docs-site/agents/`: index, checklists, forbidden, docs-maintenance.

### Phase 10 — Post-migration stabilization (2026-05-15)

Серия багов после Phase 4 (Vue extensions). См. `.agent/tasks/2026-05-15-post-migration-fixes/{spec,evidence}.md` для proof loop.

- **Drop spaces concept** (single DB per user) — welcome screen и space-picker убраны. Dashboard сразу открывается на список объектов. Делphi shim emits stub `KEPLERDEFAULT` чтобы не показывать SpaceSetup modal.
- **Master bug — `missing field 'id'`** (`services/kepler-backend/src/ws_server.rs`): backend strip'ил `id` из params как envelope id, ломал `get_object`/`delete_object`/`upsert_object` с top-level id. Fix: чтит `_req_id` для envelope, оставляет `id` нетронутым. Это разблокировало все extension CRUD.
- **Extension ARK bridge ready-gate** — `kepler:extension:ark:request` ждёт arkClient connect (15s timeout) вместо мгновенного throw. См. commit fdfc8d4.
- **Делphi task persistence** — lazy ensure `task_obj` object_type перед первым upsert. FK constraint failed → silent swallow в `.catch()` → задача жила in-memory. Fix: registered + warn вместо silent.
- **Horologion orphan filter + DesktopChrome titlebar + transitionend** — несколько мелких фиксов параллельно (mode-toggle hide, orphan time_entries, animation jank).
- **Dev mode data isolation** — `keplerDataDir()` returns `Kosmos-dev` в dev, `Kosmos` в prod, `KOSMOS_DATA_DIR` override в test. shell+backend смотрят на один dir.

### Phase 6.0 / 6.0.A — Eden как extension (2026-05-17)

См. `.agent/tasks/2026-05-17-eden-extension/spec.md` и `.agent/tasks/2026-05-17-eden-cleanup-and-hardening/spec.md`.

**Phase 6.0 — Scaffold + ARK note CRUD**:
- `extensions/eden/` создан как Vue extension (manifest, package, vite config, src/).
- `kepler-api-shim` (renderer-side bridge поверх `window.kepler.ark.request`) — emulates `window.api` так, что Eden codebase почти не правился.
- Note CRUD / folders / typed-notes / search — все ARK операции через shim.
- Команды `eden:note:create` / `eden:note:search` зарегистрированы в command bus.

**Phase 6.0.A — Cleanup + hardening**:
- Hevy полностью удалён (UI + API + ConnectedAppsSettings.vue + lib/hevy.ts). Замена — Olympia.
- Code lint/format удалён полностью (Editor.vue вызовы, settings panel, shim methods, vite-env types).
- Trash UI реализован поверх ARK soft-delete (`deletedAt != null` фильтр; restore через `upsert_object` с `deletedAt: null`).
- Bundle codesplit: lazy `Editor.vue` через `defineAsyncComponent` → main bundle **353KB** (gzip 112KB), editor chunk 1.36MB lazy.
- Standalone `apps/eden/ts/` (вместе с Heart Rust + main process + preload) удалён полностью.
- Workspace + tooling cleanup: `package.json`, `Cargo.toml`, `lefthook.yml`, scripts/*.mjs.

### Arrancador full completion (2026-05-18)

См. `.agent/tasks/2026-05-18-arrancador-full-completion/spec.md`.

- Backend `services/kepler-backend/src/arrancador/` — 5 modules (scanner, launcher, rawg, sqoba, config), 32 unit tests + 3 integration tests с synthetic Steam library (Dota 2 / Cairn / Outlast).
- Scanner: Steam VDF/ACF custom parser (без новых deps) + Epic JSON manifests. GOG skip.
- Launcher: `steam://rungameid/<id>` через `cmd /c start` + прямой exe spawn для Epic/manual.
- RAWG client: search + get_details + apply (merge в game_obj.propsJson), httpmock тесты.
- SQOBA: discover save paths heuristics, zip с `_sqoba_meta.json`, restore с path traversal protection, rotation keep N=10.
- WS namespace `arrancador.*` (scan/launch/rawg.*/sqoba.*/config.*).
- UI: все 4 stub'нутые страницы оживлены (Library launch button, Scan кнопка + history, Catalogue RAWG search+apply Modal, Sqoba per-game backup/restore, Settings RAWG key).
- Preload bridge `window.kepler.arrancador.*`.
- cargo test 110/110, typecheck/build/ark-guard зелёные.

### Phase 7 — Universal per-type data export (2026-05-18)

См. `.agent/tasks/2026-05-18-phase-7-universal-export/spec.md` и `docs-site/concepts/data-export.md`.

- Rust `Converter` trait + registry в `services/kepler-backend/src/export/` + WS endpoints `export.list` / `export.run`.
- 6 первых конвертеров: `note_md` (TipTap → markdown с YAML frontmatter), `task_md`, `task_csv`, `time_entry_csv`, `tag_json`, `game_json`.
- Shell UI: новый таб «Экспорт» в `shell/src/views/SettingsView.vue` — per-converter карта, native directory picker, история экспортов (10 шт в localStorage).
- 17 unit tests для converters; cargo test 75/75 зелёный.
- ARK guard:writes остался clean (export — read-only через `list_objects_by_type`).
- Eden export-to-markdown заменён этим универсальным механизмом.

### Lock-file test isolation (2026-05-17)

env-флаг `KOSMOS_LOCK_PERMISSIONS_DISABLED=1` в `services/kepler-backend/src/lock_file.rs` пропускает icacls/chmod ACL-хардинг в test mode. `tests/e2e/helpers/launch.ts` выставляет автоматически. Решает проблему stale ACL lock-файлов при смене Windows account'а.

### Visuals unification (2026-05-18)

3 новых компонента в `@kosmos/visuals`: `Toggle`, `SettingsRow`, `EmptyState`. 4 новых tokens (`--destructive-foreground`, `--status-warning`, `--status-connecting`, `--scrim-gradient`). Все 4 extensions мигрированы где возможно (Horologion StatusDot оставлен для e2e compat, Arrancador Tailwind plugin сохранён транзитивно через GamePosterCard).

### Phase 10 — Playwright e2e infrastructure

`tests/e2e/` — 13 specs, single worker, isolated DB per spec под `tests/.e2e/<slug>/`. Helper `tests/e2e/helpers/launch.ts` refuses paths inside `%APPDATA%`. Backend читает `KOSMOS_DATA_DIR` env override.

```powershell
bun run test:e2e            # full suite (~1.5min, 13/13 PASS)
bun run test:e2e:headed     # visible Electron
bunx playwright test --list # parse-check
```

Покрытие: launcher boot, Делphi sidebar + tasks + persistence, Horologion stopwatch/pomodoro/persistence/toggle-hide, extension ARK bridge ready race.

### Migration scripts

- `scripts/migrate-kepler-to-kosmos.ps1` — user data `%APPDATA%\Kepler` → `%APPDATA%\Kosmos` (atomic Move-Item + lock-файл renames + HKCU Run update).
- `scripts/migrate-kepler-to-kosmos-smoke.ps1` — isolated smoke test (16/16 assertions pass).
- `scripts/check-swap-completeness.ps1` — grep audit forbidden token patterns.
- `scripts/measure-kepler-ram.ps1` — baseline / kepler / `-Compare` modes для RAM benchmarks.
- `scripts/fix-mojibake.mjs` — UTF-8 recovery после PowerShell encoding bugs.
- `scripts/merge-swap.mjs` — token swap helper после `git checkout --theirs` merge conflicts.
- `scripts/dev-extensions.mjs` — orchestrator для Vite dev servers per extension.

## ⏳ Не сделано / отложено

### Phase 6 — Eden migration

**Размер**: ~2 недели сфокусированной работы. Eden намеренно отложен — TipTap editor + Heart Rust sidecar (vault filesystem manager) + сложный preload API (titlebar history, store hardening, FTS5, sync ops). Eden остаётся `Eden.exe` standalone + общий backend через cosmos-mode.

Ожидаемый RAM save после migration: ~250 MB.

### Phase 7 — Adaptive lifecycle (optional)

LRU eviction, RAM budget management, lazy extension load/unload. Имеет смысл только если open extensions >> память бюджет — для 4 текущих не критично.

### Phase 8 — Retire legacy

После production smoke testing:
- `apps/kepler/` (старый Rust gpui launcher) — удалить.
- `apps/{dashboard,delphi,horologion,arrancador}/` standalone Electron — удалить (extensions cover everything).
- Eden — оставить пока не сделано Phase 6.
- Auto-update mechanism (`electron-updater`) — **подключён** (Phase 8b, 2026-05-16): `shell/electron/autoupdater-host.ts` (state machine: idle/checking/available/downloading/downloaded/error), Raycast-style banner в Settings, launcher-команда `kepler:check-updates`, кнопка «Проверить обновления» в General. Distribution через `yoso-industries/kepler-releases`. См. [Distribution](docs-site/concepts/distribution.md#kepler-launcher-autoupdater).

### Phase 9 — Delphi UI на plain CSS (open question)

Delphi extension использует **Tailwind v4** (наследие legacy `apps/delphi/ts/`). Все остальные extension'ы + Kepler launcher / settings — на **plain scoped CSS + `@kosmos/visuals` CSS variables**.

Что нужно для Phase 9 (open question):

- ~30 .vue файлов в `apps/kepler-shell/extensions/delphi/src/` — удалить Tailwind utility classes из templates.
- Переписать стили в `<style scoped>` с CSS vars из `@kosmos/visuals`.
- Удалить `@import "tailwindcss"` + `@source` из `extensions/delphi/src/global.css`.
- Удалить `@tailwindcss/vite` plugin из vite configs.
- Удалить tailwind deps.

Скоуп — несколько часов сфокусированной работы. Откладывается до момента когда Delphi UI стабилизируется.

### Phase 10+ — Dynamic extension store

- Manifest URL discovery (extension может объявить `updateUrl`).
- Download + install third-party extensions без переустановки Kepler.
- Permission model (которые ARK operations extension может вызывать).
- Extension marketplace / signing — long-term.

Сейчас все 4 extensions ship'ятся вместе с Kepler как extra resources.

### Что ещё в TODO

- **Arrancador**: native scanner integration (либо migrate в `kepler-backend`, либо отдельный sidecar spawned by kepler-shell).
- **Dashboard**: aggregation в renderer'е (group/sum по usage_sessions).
- **Arrancador**: RAWG metadata fetch (HTTP proxy через kepler-backend).
- **Game launch** (Arrancador): нужен `kepler.window.launch(exe)` API в shell.
- **Manual smoke test infrastructure**: Playwright skeleton есть, реальные тесты для extensions не написаны.
- **Real RAM benchmark в prod mode** (без auto DevTools).

## Команды

### Dev workflow (Raycast-style)

```cmd
:: Terminal 1: Vite dev servers per extension с HMR
bun run --cwd apps/kepler-shell dev:extensions
::   → dashboard:  http://localhost:5180/
::   → horologion: http://localhost:5181/
::   → delphi:     http://localhost:5182/
::   → arrancador: http://localhost:5183/

:: Terminal 2: Kepler shell + main process
bun run --cwd apps/kepler-shell dev:kepler

:: или для проверки prod build:
bun run --cwd apps/kepler-shell dev
```

### Production build

```cmd
cd apps\kepler-shell
bun install
bun run build
:: → release/Kepler Setup 0.1.6.exe (3-5 min cold)
```

### Verify

```cmd
:: Backend Rust + 46 unit tests
cargo build --manifest-path services/kepler-backend/Cargo.toml --bin kepler-backend
cargo test --manifest-path services/kepler-backend/Cargo.toml --lib

:: TypeScript
bun run --cwd packages/kosmos-ark typecheck
bun run --cwd apps/kepler-shell typecheck

:: Extensions build
bun run --cwd apps/kepler-shell build:extensions

:: RAM benchmark
pwsh scripts/measure-kepler-ram.ps1 -Mode baseline
pwsh scripts/measure-kepler-ram.ps1 -Mode kepler
pwsh scripts/measure-kepler-ram.ps1 -Compare
```

### Migration (если ещё не сделан)

```cmd
pwsh scripts/migrate-kepler-to-kosmos.ps1 -WhatIf   :: preview
pwsh scripts/migrate-kepler-to-kosmos.ps1           :: execute
```

## История ветки kosmos/phase-1-scaffold

```
402f931 polish (Tailwind / Delphi shim / Arrancador pages) + NSIS packaging
6927cd5 mtime-based icon cache invalidation
6f09086 crash on close + app icons + status dot + selected-space DB
54fc0fe extension dev mode (Raycast-style HMR) + docs catchup
96fe2fa Phase 4 — 4 apps migrated as Vue extensions inside Kepler
7cb16df __dirname ESM shim fix
35b0134 real handlers + settings + extension loader PoC
af24795 Phase 2 command bus full stack
8b2948f Phase 1 finishing (WS, resize, state, smoke, RAM)
0280bdb Phase 1 scaffold Electron Kepler launcher
66df4ce merge main (Horologion streamer + audit + Dropdown)
de064e0 global swap Kepler ↔ Kosmos (836 файлов)
000034e Phase 0 + Phases 1-6 legacy Rust scaffold
```
