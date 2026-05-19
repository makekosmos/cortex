# Стек и инструменты

## Общий стек

### Языки и runtime

| Слой | Инструменты | Где используется |
|---|---|---|
| Системный язык | **Rust** (stable) + **Cargo workspace** | `crates/ark-core`, `services/kepler-backend`, `services/kepler-focus-svc`, `services/kepler-focus-helper`, `services/kepler-watcher`, `services/ark-relay-server` |
| Async runtime (Rust) | **tokio** (multi-threaded) | весь backend / WS server / spawning subprocess |
| Type system (TS) | **TypeScript** (`tsc --noEmit`) | shell + все Vue extensions |
| Desktop UI | **Vue 3.6.0-beta.12** (везде) + Vapor (experimental, активно в Eden) | Kepler shell + все 4 Vue-extensions (Eden, Horologion, Delphi, Arrancador) + Dashboard |
| Desktop shell | **Electron 41** | Kepler launcher + extension windows |
| Android UI | **Kotlin** + **Jetpack Compose** + **Gradle** + **Room** (ContentProvider) | `mobile/delphi` (UI), `mobile/ark-service` (Room ContentProvider для Delphi mobile) |
| Cross-language FFI | **UniFFI** (Mozilla) | `crates/ark-core` → Android Room provider; единый ARK code path |

### Менеджеры пакетов и сборка

| Слой | Инструменты | Где используется |
|---|---|---|
| Пакет-менеджер / runner (TS) | **Bun 1.3.5** (workspaces) | весь TS-монорепо |
| Пакет-менеджер (Rust) | **Cargo workspace** | корневой `Cargo.toml` + per-crate |
| Bundler (TS) | **Vite 8** + **Rolldown 1.0.1** (production-stable) | shell renderer, extension dist'ы, docs-site |
| Electron integration в Vite | **vite-plugin-electron** + **vite-plugin-electron-renderer** | shell — Vite билдит main process + preload через electron-plugin |
| Installer / packaging | **electron-builder** + **electron-updater** (auto-update) | `.exe` NSIS-installer Kepler, publish в GitHub Releases, auto-update flow |
| Installer assets | **electron-rebuild** (native deps), **rcedit** (Windows exe metadata), **png-to-ico** (icon conversion) | подготовка `Kepler-Setup-X.X.X.exe` |
| Code signing | **signtool.exe** (Windows) | в `electron-builder` pipeline для Authenticode |

### Качество кода

| Слой | Инструменты | Где используется |
|---|---|---|
| Lint (TS/JS) | **oxlint** (oxc suite, Rust-based) | TS-пакеты и приложения |
| Format (TS/JS) | **oxfmt** | TS, JS, JSON, Markdown |
| Cargo workspace hygiene | **cargo-shear** (Boshen) — fail в pre-push если orphan-deps | `Cargo.toml`'ы, lefthook pre-push |
| TypeScript check | **`tsc --noEmit`** | shell + extensions, lefthook pre-commit |
| Pre-commit / pre-push hooks | **lefthook** | весь репо (cargo check + tsc + nextest + shear) |

### Тестирование

| Слой | Инструменты | Где используется |
|---|---|---|
| Unit (TS, pure JS) | **`bun test`** | `extensions/*/tests/*.test.ts` |
| Component (Vue, real Chromium) | **Vitest 4** + **`@vitest/browser`** + **`@vitest/browser-playwright`** + **`vitest-browser-vue`** + **`@vitejs/plugin-vue`** — real Chromium, не jsdom (для Vapor edge-cases) | `extensions/eden/tests/components/*.spec.ts` |
| Unit (Rust) | **cargo-nextest** (`cargo nextest run`) | все workspace crates |
| Property-based (Rust) | **proptest** | sync invariants, ARK CRDT properties |
| Benchmarks (Rust) | **criterion** | `crates/ark-core/benches/` (pomodoro_session, delphi_filters) |
| HTTP mocking (Rust) | **httpmock** | relay-server / arrancador RAWG integration tests |
| E2E (Electron) | **Playwright** + **`@playwright/test`** через `_electron.launch` | `tests/e2e/*.spec.ts` (eden, delphi, horologion, arrancador, launcher, commands-architecture) |
| Watch-mode (Rust, опционально) | **bacon** (cargo-watch deprecated) | personal dev-tool, `cargo install bacon` локально |

### Данные, хранилище, сериализация

| Слой | Инструменты | Где используется |
|---|---|---|
| ARK storage (Rust) | **SQLite** через **`rusqlite`** (bundled, с **FTS5**) | `crates/ark-core/rust` — единый storage для всех apps |
| Electron main каши | **better-sqlite3** | shell main process (window state, не-ARK кэши) |
| Full-text search | **ARK FTS5** через `search_objects` op | Eden (будущий semantic search — Phase 16 в [roadmap](/apps/kepler-roadmap)) |
| Сериализация (Rust) | **serde** + **serde_json** | весь Rust workspace — wire format ARK protocol |
| Runtime schema validation (TS) | **zod** | Eden (validation `note_type::schema_json` / `header_props`) |
| Identifiers | **uuid** (Rust + TS) | entry IDs, object IDs, device IDs |

### State management (Vue)

| Слой | Инструменты | Где используется |
|---|---|---|
| Reactive state | **Pinia 3** | Eden, Horologion (stores `useEdenStore`, `usePomodoroStore`) |
| Server state cache (планируется) | **@pinia/colada 1.3** — установлен, миграция в Phase 14 | Eden state migration — см. [Roadmap Phase 14](/apps/kepler-roadmap) |
| Routing | **vue-router** (memory history) | Delphi (5 pages), при необходимости в других extensions |

### Сетевая инфраструктура (LAN sync, IPC)

| Слой | Инструменты | Где используется |
|---|---|---|
| WS server (Rust) | **tokio-tungstenite** через `kepler-backend` | command bus + ARK protocol на `127.0.0.1:<random>` |
| WS client (TS) | **`ws`** (Node WebSocket) | `@kosmos/ark` SDK + Delphi |
| LAN discovery | **bonjour-service** (mDNS) | Delphi peer-discovery в LAN |
| Network interfaces (Rust) | **if-addrs** | LAN sync — определение routable IPv4/IPv6 |
| HTTP client (Rust) | **reqwest** | Arrancador RAWG API, future external integrations |
| QR codes | **qrcode** | Delphi sync — QR с peer connection info |
| URL encoding (Rust) | **urlencoding** | links в notifications / deep-links |

### Windows-specific

| Слой | Инструменты | Где используется |
|---|---|---|
| Windows API bindings (Rust) | **windows-rs** (`windows` crate) | hosts file modification (focus-helper), service control, system info |
| Windows Service framework | **windows-service** | `kepler-focus-svc` — SCM register / lifecycle / pipe IPC |
| Manifest embedding | **embed_manifest** | `kepler-focus-helper` (`requireAdministrator`), `kepler-focus-svc` (`asInvoker`) |

### Editor stack (Eden)

| Слой | Инструменты | Где используется |
|---|---|---|
| Rich-text core | **TipTap 3** на ProseMirror (через **`@tiptap/pm`**) + **`@tiptap/starter-kit`** | Eden editor |
| Markdown serialize | **`@tiptap/markdown`** | export, copy markdown |
| Mentions / wikilinks | **`@tiptap/extension-mention`** + **`@tiptap/suggestion`** | slash command menu, wikilink autocomplete |
| Placeholder | **`@tiptap/extension-placeholder`** | empty doc hint |
| Code blocks | **`@tiptap/extension-code-block-lowlight`** + **lowlight** | syntax highlighting code blocks |
| Typography | **`@tiptap/extension-typography`** | smart quotes / dashes |
| Popups (slash menu, mentions) | **tippy.js** | rendering suggestion popup |

### UI shared

| Слой | Инструменты | Где используется |
|---|---|---|
| UI tokens / shared components | **`@kosmos/visuals`** (workspace) | shell + все Vue extensions |
| Icons | **`lucide-vue-next`** | все Vue extensions + shell |
| Fonts | **`@fontsource-variable/inter`** + **`@fontsource/ibm-plex-mono`** | через `@kosmos/visuals` |
| Tailwind стек (legacy Delphi) | **tailwindcss** + **`@tailwindcss/vite`** + **reka-ui** + **clsx** + **class-variance-authority** + **tailwind-merge** | **Только Delphi** — переезд на plain CSS в [Phase 9](/apps/kepler-roadmap#phase-9) |

### Component docs

| Слой | Инструменты | Где используется |
|---|---|---|
| Visual catalog | **Storybook 10** + **histoire** | `packages/visuals` для документации компонентов |

### Документация (docs-site)

| Слой | Инструменты | Где используется |
|---|---|---|
| Docs framework | **VitePress** (тот сайт что ты сейчас читаешь) | `docs-site/` |
| Diagrams | **mermaid** + **svg-pan-zoom** (lazy-loaded) | concepts, roadmap |

### Release tooling (external)

| Слой | Инструменты | Где используется |
|---|---|---|
| GitHub CLI | **`gh`** | `shell/scripts/publish-extension.mjs`, marketplace `ext:catalog`, manual releases |
| Archive (Rust) | **zip** crate | `.kext` extension package format extraction |
| CSV export | **csv** crate | Phase 7 universal export — time entries / tags → CSV |

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
