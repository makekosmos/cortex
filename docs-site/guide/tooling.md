# Стек и инструменты

## Общий стек

| Слой | Инструменты | Где используется |
|---|---|---|
| Пакет-менеджер / runner | **Bun 1.3.5** | весь монорепо |
| Системный язык | **Rust** (stable) + Cargo | `crates/ark-core`, `services/kepler-backend/src/usage_tracker` |
| Android | **Kotlin** + Gradle + Compose + Room | `mobile/delphi` (UI), `mobile/ark-service` (Room ContentProvider) |
| Desktop UI | **Vue 3.6.0-beta.12** (везде) + Vapor (experimental, активно в Eden) | Kepler shell + все 4 Vue-extensions (Eden, Horologion, Delphi, Arrancador) + Dashboard |
| Desktop shell | **Electron 41** | Kepler launcher + extension windows (Eden, Delphi, Arrancador, Horologion, Dashboard) |
| Bundler | **Vite 8** (Rolldown) | TS-приложения |
| Линт / формат | **oxlint** + **oxfmt** (oxc suite, Rust-based) | TS/JS |
| Unit-тесты | **Vitest** | TS-пакеты и приложения |
| E2E | **Playwright** | Electron-приложения |
| Pre-commit hooks | **lefthook** | весь репо |
| Storage (Electron) | **better-sqlite3** | Electron main процессы |
| State (desktop) | **Pinia** | Eden |
| Search (Eden) | **ARK FTS5** (SQLite встроенный full-text search) через `search_objects` op | Eden |
| Editor (Eden) | **TipTap** | Eden |

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
