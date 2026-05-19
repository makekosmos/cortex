# Стек и инструменты

## Общий стек

### Языки и runtime

| Слой | Инструменты | Где используется |
|---|---|---|
| Системный язык | **Rust** (stable) + Cargo | `crates/ark-core`, `services/kepler-backend`, `services/kepler-focus-svc`, `services/kepler-focus-helper` |
| Desktop UI | **Vue 3.6.0-beta.12** (везде) + Vapor (experimental, активно в Eden) | Kepler shell + все 4 Vue-extensions (Eden, Horologion, Delphi, Arrancador) + Dashboard |
| Desktop shell | **Electron 41** | Kepler launcher + extension windows |
| Android | **Kotlin** + Gradle + Compose + Room | `mobile/delphi` (UI), `mobile/ark-service` (Room ContentProvider) |

### Менеджеры пакетов и сборка

| Слой | Инструменты | Где используется |
|---|---|---|
| Пакет-менеджер / runner (TS) | **Bun 1.3.5** | весь TS-монорепо (workspaces + scripts) |
| Пакет-менеджер (Rust) | **Cargo** (workspace) | `Cargo.toml` в корне + per-crate |
| Bundler (TS) | **Vite 8** (Rolldown 1.0.1 — production-stable) | shell renderer, extension dist'ы, docs-site |
| Электронный installer | **electron-builder** (`shell/package.json::build`) + **electron-updater** для auto-update | Kepler installer (`.exe` NSIS) + auto-update флоу |

### Качество кода

| Слой | Инструменты | Где используется |
|---|---|---|
| Lint / format (TS/JS) | **oxlint** + **oxfmt** (oxc suite, Rust-based) | TS-пакеты и приложения |
| Cargo workspace hygiene | **cargo-shear** (тот же Boshen что oxc) — fail в pre-push если найдены orphan-deps | проверка `Cargo.toml` в lefthook pre-push |
| TypeScript | `tsc --noEmit` | shell + extensions, в lefthook pre-commit |
| Pre-commit / pre-push hooks | **lefthook** | весь репо |

### Тестирование

| Слой | Инструменты | Где используется |
|---|---|---|
| Unit (TS, pure JS) | **`bun test`** | `extensions/*/tests/*.test.ts` (charCount, lib/*) |
| Component (Vue, real Chromium) | **Vitest 4** + **`@vitest/browser-playwright`** + **`vitest-browser-vue`** — не jsdom (нужно для Vapor edge-cases) | `extensions/eden/tests/components/*.spec.ts` |
| Unit (Rust) | **cargo-nextest** (`cargo nextest run`) | все workspace crates, lefthook pre-push |
| E2E (Electron) | **Playwright** через `_electron.launch` | `tests/e2e/*.spec.ts` (eden, delphi, horologion, arrancador, launcher) |
| Watch-mode (Rust, опционально) | **bacon** (cargo-watch deprecated, его мейнтейнер рекомендует bacon) | personal dev-tool, не проектный артефакт — `cargo install bacon` локально |

### Данные и состояние

| Слой | Инструменты | Где используется |
|---|---|---|
| Storage (ARK) | **SQLite (FTS5)** через `rusqlite` (Rust) | `crates/ark-core` runtime — единый ARK storage для всех apps |
| Storage (Electron main) | **better-sqlite3** | shell main process для local non-ARK кэшей (e.g., window state) |
| Search | **ARK FTS5** через `search_objects` op | Eden, потенциально другие apps |
| State (Vue) | **Pinia** (миграция на **Pinia Colada** для server-state — Phase 14, см. [Roadmap](/apps/kepler-roadmap)) | Eden, Horologion |

### Специфичное для приложений

| Слой | Инструменты | Где используется |
|---|---|---|
| Rich-text editor | **TipTap 3** (ProseMirror) + `@tiptap/markdown` для serialize | Eden |
| Icons | **`lucide-vue-next`** | все Vue extensions + shell |
| Themes / UI tokens | **`@kosmos/visuals`** (workspace package) | все Vue extensions + shell |

## Гварды и smoke-скрипты

Эти команды живут в корне репо и должны прогоняться **до** PR с изменениями в data-слое:

```powershell
bun run ark:guard:writes
# scripts/check-ark-write-boundaries.mjs
# Запрещает прямые SQL writes в ARK SQLite из app services.

bun run ark:smoke
# scripts/ark-smoke.mjs
# Прогоняет smoke matrix по всем компонентам с изолированными БД.
```

## Линт и формат

```powershell
bun run lint        # запускается per-workspace, oxlint
bun run format      # oxfmt --check
bun run typecheck   # tsc --noEmit (в TS-пакетах)
```

`lefthook` поднимает форматтер и линт автоматически на pre-commit. Не выключай хуки `--no-verify`; если хук падает — чини причину.

## Сборка ARK runtime

```powershell
cargo build --workspace                                                 # все crates + services
cargo build --manifest-path crates\ark-core\rust\Cargo.toml --bin ark-core-rpc
cargo test  --manifest-path crates\ark-core\rust\Cargo.toml
bun run --cwd packages/ark typecheck
bun run --cwd packages/ark build
```

## Сборка Kepler shell (главный путь)

```powershell
cd shell
bun run build:backend:dev    # cargo build (debug) services/kepler-backend
bun run dev                  # backend + extensions + Vite + Electron
bun run build:js             # tsc + vite + extensions (без NSIS)
bun run build                # release backend + js + electron-builder --win nsis
bun run typecheck            # tsc --noEmit
bun run test:e2e             # Playwright
```

## Сборка Eden

Eden — Vue extension внутри Kepler shell, отдельной сборки не имеет (`apps/eden/ts/` standalone удалён в Phase 6.0.A). Сборка происходит как часть `shell/`:

```powershell
bun run --cwd shell build:extensions   # собирает dist/ всех Vue extensions, включая Eden
bun run --cwd shell dev                # dev shell (extension HMR — opt-in через KEPLER_DEV_EXTENSIONS=1)
```

## Документация

```powershell
bun run docs:dev           # http://localhost:5173, hot reload markdown
bun run docs:build         # статика в docs-site/.vitepress/dist
bun run docs:preview       # превью собранного
```
