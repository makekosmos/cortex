# Стек и инструменты

## Общий стек

| Слой | Инструменты | Где используется |
|---|---|---|
| Пакет-менеджер / runner | **Bun 1.3.5** | весь монорепо |
| Системный язык | **Rust** (stable) + Cargo | `packages/ark-core`, `services/usage-tracker`, `apps/eden/ts/heart` |
| Android | **Kotlin** + Gradle + Compose + Room | `apps/delphi/kotlin` (UI), `apps/ark-service` (Room ContentProvider) |
| Desktop UI | **Vue 3.6 Vapor** | Eden, Arrancador (Vue), Dashboard |
| Desktop shell | **Electron 38** | Eden, Delphi, Arrancador, Dashboard |
| Bundler | **Vite 8** (Rolldown) | TS-приложения |
| Линт / формат | **Biome**, **oxlint**, **oxfmt** | TS/JS |
| Unit-тесты | **Vitest** | TS-пакеты и приложения |
| E2E | **Playwright** | Electron-приложения |
| Pre-commit hooks | **lefthook** | весь репо |
| Storage (Electron) | **better-sqlite3** | Electron main процессы |
| State (desktop) | **Pinia** | Eden |
| Search (Eden) | **Tantivy** (Rust) через Eden Heart sidecar | Eden |
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
bun run lint        # запускается per-workspace, Biome / oxlint
bun run format      # oxfmt --check
bun run typecheck   # tsc --noEmit (в TS-пакетах)
```

`lefthook` поднимает форматтер и линт автоматически на pre-commit. Не выключай хуки `--no-verify`; если хук падает — чини причину.

## Сборка ARK runtime

```powershell
cargo build --manifest-path packages\ark-core\rust\Cargo.toml --bin ark-core-rpc
cargo test  --manifest-path packages\ark-core\rust\Cargo.toml
bun run --cwd packages/kepler-ark typecheck
bun run --cwd packages/kepler-ark build
```

## Сборка приложения (на примере Delphi)

```powershell
cd apps/delphi/ts
bun run build:ark:dev      # debug-сборка ark-core-rpc
bun run build:ark          # release-сборка ark-core-rpc
bun run dev                # build:ark:dev + Vite + Electron
bun run build              # build:ark + TS + Vite + electron-builder
bun run test               # unit
bun run e2e                # Playwright
```

## Документация

```powershell
bun run docs:dev           # http://localhost:5173, hot reload markdown
bun run docs:build         # статика в docs-site/.vitepress/dist
bun run docs:preview       # превью собранного
```
