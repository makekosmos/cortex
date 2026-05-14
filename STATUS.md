# Kosmos — статус проекта (2026-05-14)

Итог архитектурного pivot'а от standalone Electron-апок к Kepler-host архитектуре с Vue extensions. Замигрированы 4 из 5 апок (Eden намеренно отложен).

## Архитектура

```
Kepler.exe (Electron host)
  ├─ launcher window (Ctrl+Shift+K, fixed 720×460, acrylic)
  ├─ settings window (tray menu)
  ├─ extension-host
  │   ├─ extensions/dashboard/   (Vue bundle inside Kepler)
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
| TS package SDK | `@kepler/ark` |
| Visuals (CSS tokens + Vue components) | `@kepler/visuals` |
| Backend binary | `kepler-backend.exe` |

`%APPDATA%\Kosmos\` — все user data (ark.db, lock-файлы, settings). `%APPDATA%\Kosmos\spaces\<spaceId>\ark.db` — actual per-space DB.

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
- `@kepler/ark` SDK: `client.commands.{register,unregister,list,invoke,onInvoked,onChanged}` namespace + types (`CommandManifest`, `CommandInvokedEvent`).
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
- `bun run --cwd apps/kepler-shell build` → `release/Kepler Setup 0.0.1.exe`.
- Install path: `%LOCALAPPDATA%\Programs\Kepler\` (per-user oneClick).

### Inter-app communication

- `@kepler/ark` cosmos-mode — apps подключаются к single `kepler-backend` WS, делят `ark-core-rpc`. Один sync node на машину (-3 ark-core-rpc).
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

- `docs-site/concepts/`: architecture, command-bus, extension-host, extension-dev-mode, ram-benchmarks, sync, ark-objects, write-boundary.
- `docs-site/apps/`: kepler, kepler-roadmap, horologion, delphi, dashboard, arrancador.
- `docs-site/packages/`: kosmos-ark (с Commands API), kosmos-visuals, ark-core.
- `docs-site/reference/`: commands, rules, decisions, smoke-matrix.
- `docs-site/agents/`: index, checklists, forbidden, docs-maintenance.

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
- Auto-update mechanism (`electron-updater`) — не подключён.

### Phase 9 — Delphi UI на plain CSS (open question)

Delphi extension использует **Tailwind v4** (наследие legacy `apps/delphi/ts/`). Все остальные extension'ы + Kepler launcher / settings — на **plain scoped CSS + `@kepler/visuals` CSS variables**.

Что нужно для Phase 9 (open question):

- ~30 .vue файлов в `apps/kepler-shell/extensions/delphi/src/` — удалить Tailwind utility classes из templates.
- Переписать стили в `<style scoped>` с CSS vars из `@kepler/visuals`.
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
:: → release/Kepler Setup 0.0.1.exe (3-5 min cold)
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
