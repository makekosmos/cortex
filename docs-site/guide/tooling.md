# Стек и инструменты

## Общий стек

### Языки и runtime

| Слой                 | Инструменты                                                                | Где используется                                                                                                                                                  |
| -------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Системный язык       | **Rust** (stable) + **Cargo workspace**                                    | `crates/ark-core`, `services/kepler-backend`, `services/kepler-focus-svc`, `services/kepler-focus-helper`, `services/kepler-watcher`, `services/ark-relay-server` |
| Async runtime (Rust) | **tokio** (multi-threaded)                                                 | весь backend / WS server / spawning subprocess                                                                                                                    |
| Type system (TS)     | **TypeScript** (`tsc --noEmit`)                                            | shell + все Vue extensions                                                                                                                                        |
| Desktop UI           | **Vue 3.6.0-beta.12** (везде) + Vapor (experimental, активно в Eden)       | Kepler shell + все 4 Vue-extensions (Eden, Horologion, Delphi, Arrancador) + Dashboard                                                                            |
| Desktop shell        | **Electron 41**                                                            | Kepler launcher + extension windows                                                                                                                               |
| Android UI           | **Kotlin** + **Jetpack Compose** + **Gradle** + **Room** (ContentProvider) | `mobile/delphi` (UI), `mobile/ark-service` (Room ContentProvider для Delphi mobile)                                                                               |
| Cross-language FFI   | **UniFFI** (Mozilla)                                                       | `crates/ark-core` → Android Room provider; единый ARK code path                                                                                                   |

### Менеджеры пакетов и сборка

| Слой                         | Инструменты                                                                                             | Где используется                                                          |
| ---------------------------- | ------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Пакет-менеджер / runner (TS) | **Bun 1.3.5** (workspaces)                                                                              | весь TS-монорепо                                                          |
| Пакет-менеджер (Rust)        | **Cargo workspace**                                                                                     | корневой `Cargo.toml` + per-crate                                         |
| Bundler (TS)                 | **Vite 8** + **Rolldown 1.0.1** (production-stable)                                                     | shell renderer, extension dist'ы, docs-site                               |
| Electron integration в Vite  | **vite-plugin-electron** + **vite-plugin-electron-renderer**                                            | shell — Vite билдит main process + preload через electron-plugin          |
| Installer / packaging        | **electron-builder** + **electron-updater** (auto-update)                                               | `.exe` NSIS-installer Kepler, publish в GitHub Releases, auto-update flow |
| Installer assets             | **electron-rebuild** (native deps), **rcedit** (Windows exe metadata), **png-to-ico** (icon conversion) | подготовка `Kepler-Setup-X.X.X.exe`                                       |
| Code signing                 | **signtool.exe** (Windows)                                                                              | в `electron-builder` pipeline для Authenticode                            |

### Качество кода

| Слой                        | Инструменты                                                 | Где используется                                |
| --------------------------- | ----------------------------------------------------------- | ----------------------------------------------- |
| Lint (TS/JS)                | **oxlint** (oxc suite, Rust-based)                          | TS-пакеты и приложения                          |
| Format (TS/JS)              | **oxfmt**                                                   | TS, JS, JSON, Markdown                          |
| Cargo workspace hygiene     | **cargo-shear** (Boshen) — fail в pre-push если orphan-deps | `Cargo.toml`'ы, lefthook pre-push               |
| TypeScript check            | **`tsc --noEmit`**                                          | shell + extensions, lefthook pre-commit         |
| Pre-commit / pre-push hooks | **lefthook**                                                | весь репо (cargo check + tsc + nextest + shear) |

### Тестирование

| Слой                           | Инструменты                                                                                                                                                                    | Где используется                                                                              |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| Unit (TS, pure JS)             | **`bun test`**                                                                                                                                                                 | `extensions/*/tests/*.test.ts`                                                                |
| Component (Vue, real Chromium) | **Vitest 4** + **`@vitest/browser`** + **`@vitest/browser-playwright`** + **`vitest-browser-vue`** + **`@vitejs/plugin-vue`** — real Chromium, не jsdom (для Vapor edge-cases) | `extensions/eden/tests/components/*.spec.ts`                                                  |
| Unit (Rust)                    | **cargo-nextest** (`cargo nextest run`)                                                                                                                                        | все workspace crates                                                                          |
| Property-based (Rust)          | **proptest**                                                                                                                                                                   | sync invariants, ARK CRDT properties                                                          |
| Benchmarks (Rust)              | **criterion**                                                                                                                                                                  | `crates/ark-core/rust/benches/` (pomodoro_session, delphi_filters)                                 |
| HTTP mocking (Rust)            | **httpmock**                                                                                                                                                                   | relay-server / arrancador RAWG integration tests                                              |
| E2E (Electron)                 | **Playwright** + **`@playwright/test`** через `_electron.launch`                                                                                                               | `tests/e2e/*.spec.ts` (eden, delphi, horologion, arrancador, launcher, commands-architecture) |
| Watch-mode (Rust, опционально) | **bacon** (cargo-watch deprecated)                                                                                                                                             | personal dev-tool, `cargo install bacon` локально                                             |

### Данные, хранилище, сериализация

| Слой                           | Инструменты                                           | Где используется                                                            |
| ------------------------------ | ----------------------------------------------------- | --------------------------------------------------------------------------- |
| ARK storage (Rust)             | **SQLite** через **`rusqlite`** (bundled, с **FTS5**) | `crates/ark-core/rust` — единый storage для всех apps                       |
| Electron main каши             | **better-sqlite3**                                    | shell main process (window state, не-ARK кэши)                              |
| Full-text search               | **ARK FTS5** через `search_objects` op                | Eden (будущий semantic search — Phase 16 в [roadmap](/apps/kepler-roadmap)) |
| Сериализация (Rust)            | **serde** + **serde_json**                            | весь Rust workspace — wire format ARK protocol                              |
| Runtime schema validation (TS) | **zod**                                               | Eden (validation `note_type::schema_json` / `header_props`)                 |
| Identifiers                    | **uuid** (Rust + TS)                                  | entry IDs, object IDs, device IDs                                           |

### State management (Vue)

| Слой                             | Инструменты                                             | Где используется                                                    |
| -------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------- |
| Reactive state                   | **Pinia 3**                                             | Eden, Horologion (stores `useEdenStore`, `usePomodoroStore`)        |
| Server state cache (планируется) | **@pinia/colada 1.3** — установлен, миграция в Phase 14 | Eden state migration — см. [Roadmap Phase 14](/apps/kepler-roadmap) |
| Routing                          | **vue-router** (memory history)                         | Delphi (5 pages), при необходимости в других extensions             |

### Сетевая инфраструктура (LAN sync, IPC)

| Слой                      | Инструменты                                  | Где используется                                   |
| ------------------------- | -------------------------------------------- | -------------------------------------------------- |
| WS server (Rust)          | **tokio-tungstenite** через `kepler-backend` | command bus + ARK protocol на `127.0.0.1:<random>` |
| WS client (TS)            | **`ws`** (Node WebSocket)                    | `@kosmos/ark` SDK + Delphi                         |
| LAN discovery             | **bonjour-service** (mDNS)                   | Delphi peer-discovery в LAN                        |
| Network interfaces (Rust) | **if-addrs**                                 | LAN sync — определение routable IPv4/IPv6          |
| HTTP client (Rust)        | **reqwest**                                  | Arrancador RAWG API, future external integrations  |
| QR codes                  | **qrcode**                                   | Delphi sync — QR с peer connection info            |
| URL encoding (Rust)       | **urlencoding**                              | links в notifications / deep-links                 |

### Windows-specific

| Слой                        | Инструменты                      | Где используется                                                                 |
| --------------------------- | -------------------------------- | -------------------------------------------------------------------------------- |
| Windows API bindings (Rust) | **windows-rs** (`windows` crate) | hosts file modification (focus-helper), service control, system info             |
| Windows Service framework   | **windows-service**              | `kepler-focus-svc` — SCM register / lifecycle / pipe IPC                         |
| Manifest embedding          | **embed_manifest**               | `kepler-focus-helper` (`requireAdministrator`), `kepler-focus-svc` (`asInvoker`) |

### Editor stack (Eden)

| Слой                          | Инструменты                                                                      | Где используется                          |
| ----------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------- |
| Rich-text core                | **TipTap 3** на ProseMirror (через **`@tiptap/pm`**) + **`@tiptap/starter-kit`** | Eden editor                               |
| Markdown serialize            | **`@tiptap/markdown`**                                                           | export, copy markdown                     |
| Mentions / wikilinks          | **`@tiptap/extension-mention`** + **`@tiptap/suggestion`**                       | slash command menu, wikilink autocomplete |
| Placeholder                   | **`@tiptap/extension-placeholder`**                                              | empty doc hint                            |
| Code blocks                   | **`@tiptap/extension-code-block-lowlight`** + **lowlight**                       | syntax highlighting code blocks           |
| Typography                    | **`@tiptap/extension-typography`**                                               | smart quotes / dashes                     |
| Popups (slash menu, mentions) | **tippy.js**                                                                     | rendering suggestion popup                |

### UI shared

| Слой                          | Инструменты                                                                                                            | Где используется                                                                   |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| UI tokens / shared components | **`@kosmos/visuals`** (workspace)                                                                                      | shell + все Vue extensions                                                         |
| Icons                         | **`lucide-vue-next`**                                                                                                  | все Vue extensions + shell                                                         |
| Fonts                         | **`@fontsource-variable/inter`** + **`@fontsource/ibm-plex-mono`**                                                     | через `@kosmos/visuals`                                                            |
| Tailwind стек (legacy Delphi) | **tailwindcss** + **`@tailwindcss/vite`** + **reka-ui** + **clsx** + **class-variance-authority** + **tailwind-merge** | **Только Delphi** — переезд на plain CSS в [Phase 9](/apps/kepler-roadmap#phase-9) |

### Component docs

| Слой           | Инструменты      | Где используется                                                                                            |
| -------------- | ---------------- | ----------------------------------------------------------------------------------------------------------- |
| Visual catalog | **Storybook 10** | `packages/visuals` — `.stories.ts` файлы для каждого компонента, `bun run --cwd packages/visuals storybook` |

### Документация (docs-site)

| Слой           | Инструменты                                    | Где используется  |
| -------------- | ---------------------------------------------- | ----------------- |
| Docs framework | **VitePress** (тот сайт что ты сейчас читаешь) | `docs-site/`      |
| Diagrams       | **mermaid** + **svg-pan-zoom** (lazy-loaded)   | concepts, roadmap |

### Release tooling (external)

| Слой           | Инструменты   | Где используется                                                                  |
| -------------- | ------------- | --------------------------------------------------------------------------------- |
| GitHub CLI     | **`gh`**      | `shell/scripts/publish-extension.mjs`, marketplace `ext:catalog`, manual releases |
| Archive (Rust) | **zip** crate | `.kext` extension package format extraction                                       |
| CSV export     | **csv** crate | Phase 7 universal export — time entries / tags → CSV                              |

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
bun run lint            # oxlint (корневой .oxlintrc.json)
bun run format          # oxfmt — пишет в файлы
bun run format:check    # oxfmt --check (для CI / лефтхука)
bun run typecheck       # tsc --noEmit (в shell + packages/ark и пр.)
```

Активные guards (Phase 2 bug-detection, 2026-05-22):

- **`.oxlintrc.json`** в корне — `categories.correctness = error`,
  `no-unused-vars` / `no-useless-escape` / `unicorn/no-useless-fallback-in-spread`
  понижены до warn. Точечные ban'ы под workspace overrides.
- **`.oxfmtrc.json`** — defaults oxc + ignorePatterns для
  `dist/`, `.e2e/`, `legacy/`, `sample/`.
- **`Cargo.toml::[workspace.lints]`** — `clippy.unwrap_used/panic/todo/unimplemented = warn`,
  `rust.unused_must_use = deny`. Подключаются в каждом crate'е через
  `[lints]\nworkspace = true`.
- **`scripts/check-ark-write-boundaries.mjs`** — runtime invariant guard:
  SQL writes в ARK таблицы, `app.getPath('userData')` outside
  `shell/electron/instance.ts`, `path.join(..., 'Kosmos'|'Kepler', ...)`
  outside instance/data-dir, `KOSMOS_DATA_DIR=...APPDATA...` в `tests/e2e`
  outside `helpers/launch.ts`.

`lefthook` запускает на **pre-commit**: typecheck-shell, oxlint
(`{staged_files}`), oxfmt --check (`{staged_files}`), ark:guard:writes,
cargo check в crate'ах, docs:sync / docs:check.

На **pre-push**: cargo nextest, cargo shear, cargo clippy
`--workspace --all-targets`.

Не выключай хуки через `--no-verify`; если хук падает — чини причину.

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

## FAQ — почему именно эти инструменты

Здесь — обоснования ключевых выборов, чтобы не возвращаться к спорам «а может Tauri / а может Lexical / а может Biome» каждый раз когда кто-то приходит свежим взглядом. Если хочешь оспорить — открой proof loop с альтернативой и метриками.

### Почему Electron, а не Tauri / Wails / native?

::: tip Решение опирается на эксперимент
Замеры RAM/disk/start time, runnable Tauri port, полное сравнение —
[Tauri vs Electron 2026-05](/experiments/tauri-vs-electron). Краткий вывод: на
Windows экономия RAM **24%** (а не 5-10× как в маркетинге), на Linux WebKitGTK
ломает TipTap в Eden.
:::

Tauri даёт installer ~10 MB вместо ~370 MB и более «нативное» ощущение, **но это дорого по операционным затратам**:

- **RAM экономия — миф на Windows.** WebView2 — тот же Chromium что и в Electron, renderer/GPU/utility процессы жрут столько же. Реальная экономия только на main process (Node→Rust, ~135 MB). Полная разбивка по процессам — в [эксперименте](/experiments/tauri-vs-electron).
- **Кроссплатформа на Tauri — боль.** Каждая платформа использует свой WebView (WebView2 на Windows, WebKit на macOS, WebKitGTK на Linux). На Linux WebKitGTK **ломает Eden**: contentEditable в TipTap, font-weight +100 жирнее, нет WebRTC/WebGPU. На Electron Chromium везде одинаковый.
- **Тулзы экосистемы под вопросом.** Год назад Playwright под Tauri вообще не работал нормально. Сейчас [`tauri-playwright`](https://github.com/tauri-apps/awesome-tauri) подтягивается, но «работает» ≠ «работает как родной для Electron». Пока всё что есть в нашем тестовом контракте (`_electron.launch`, `page.evaluate` на main process, доступ к ARK через preload) — стабильно покрывает Electron.
- **Цена миграции.** 5814 строк TS в `shell/electron/` + 70 IPC handlers + 4 extension'а с preload bridge. Реалистично 3-6 недель работы на feature-parity на Windows + 4-8 недель на Linux/macOS обход WebKitGTK багов.
- **Не приоритет автора.** Главное сейчас — личная экосистема приложений с быстрой итерацией, не оптимизация installer size. Когда / если product станет публичным и 360 MB начнут жать — пересмотрим, **но это отдельная фаза, не текущая**.

Tauri остаётся «watch list» — следим за зрелостью Playwright/Spectron-replacement инструментов и кроссплатформенной консистентностью. Не сегодня.

### Почему Vue 3.6 beta а не stable 3.5?

Vapor Mode (no-virtual-DOM компиляция) есть только в 3.6. Для TipTap-heavy редактора Eden это ощутимый perf-win. Берём бету осознанно — Vue беты исторически стабильны (Evan You не релизит сырое), к `beta.12` API почти заморожен. Прочие 3 extension'а тоже на 3.6.0-beta.12 — единая версия проще чем фрагментация. Stable 3.6 ждём в Q1-Q2 2026.

### Почему oxc (oxlint + oxfmt), а не Biome / ESLint + Prettier?

Скорость: oxc в 50-100× быстрее ESLint, в 5× быстрее Biome на наших проектах. Один автор (Boshen) ведёт всю экосистему — oxlint, oxfmt, **cargo-shear**, плюс [`oxc-parser`](https://github.com/oxc-project/oxc) под капотом Vite/Rolldown. Когда инструменты от одной команды — меньше разъездов API. **Biome был параллельно**, но дублирование без пользы — удалили 2026-05-19.

### Почему cargo-shear, а не cargo-machete?

cargo-machete застрял на 0.6.2 (последний релиз — 2024), не понимает workspace deps (нужно для нашего multi-crate setup'а). cargo-shear от **того же Boshen что oxc** — активно развивается (`1.12.4` сейчас), `--fix` режим, использует rust-analyzer parser. Применён в oxc, rspack, rolldown, biome, uv, turbopack — production-proven.

### Почему cargo-nextest, если perf-win незаметен?

Wall-clock у нас почти не сдвинулся (тесты CPU-light, bottleneck в test-binary launch, не в исполнении). Взяли за **UX и фичи**: progress-bar, subprocess isolation, `--retries` для flaky e2e, JUnit XML для будущего CI. Замерили — [E1 в experiments](/.agent/experiments/2026-05-19-tooling-pass/baseline.md), 3.5% перерасход в шуме.

### Почему Vitest browser, а не jsdom?

Eden на Vue 3.6 beta + Vapor. **Vapor edge-cases jsdom не воспроизводит** (другой compilation target). На real Chromium через Vitest browser ловим то что иначе всплыло бы только в e2e. Плюс будущие visual regression тесты (`toMatchScreenshot`) для zen-mode / dock-corner / acrylic backdrop ложатся сюда же.

### Почему параллельно bun:test и Vitest?

Сейчас — переходный период. bun:test для pure-JS unit (charCount, lib/\*) был там до Vitest browser setup'а. Логичная консолидация — оставить только Vitest. **Запланирована** ([Phase 15-ish](/apps/kepler-roadmap)), но не приоритет — оба работают, миграция cost > benefit пока.

### Почему TipTap, а не Lexical / Slate / ProseMirror raw?

- **Lexical** (Meta) — React-first, Vue-bindings слабые/community-maintained, не подходит.
- **Slate** — Slate.js строит на ProseMirror-подобной модели но проще, **но менее зрелый** для production. Меньше extension'ов.
- **ProseMirror raw** — TipTap **поверх ProseMirror**, можно опуститься в любой момент через `@tiptap/pm`. Сразу даёт хорошие defaults (StarterKit, plugins).

TipTap 3 — Vue-нативная обёртка с богатой экосистемой плагинов (markdown, mention, code-block-lowlight, typography) — то что нужно для Eden.

### Почему Pinia + (будущая) Pinia Colada, а не TanStack Query?

TanStack Query (`@tanstack/vue-query`) — мощно, но **TanStack-родом из React**, Vue-биндинги отстают и не Vue-native в плане ergonomics. **Pinia Colada** — это **Vue-native эквивалент TanStack Query**, тот же ментальный модель (queries / mutations / cache), от core Pinia author'а (Eduardo San Martin Morote). Один автор стека = меньше разъездов API.

### Почему ARK на SQLite + FTS5, а не Postgres / SurrealDB / external?

Local-first — главный архитектурный выбор. Данные пользователя живут на его диске, синк через LAN/relay опционален. SQLite — единственный embedded engine с зрелым FTS5, ACID, и < 2MB на диске. Альтернативы (DuckDB, Turso, SurrealDB) либо не embedded, либо требуют отдельный server process.

### Почему UniFFI, а не raw JNI / NDK / cxx-rs?

Android-приложение Delphi mobile должно вызывать ту же ARK logic что desktop. Варианты:

- **JNI/NDK direct** — boilerplate, manual marshalling, опасно.
- **cxx-rs** — C++ bridge, не идиомично для Rust↔Kotlin.
- **UniFFI** (Mozilla) — declarative interface файлы, генерирует Kotlin bindings из Rust сигнатур. Используется в реальных Mozilla-приложениях (Firefox iOS, NSS).

### Почему bonjour-service (mDNS) для LAN-discovery?

mDNS — zero-config LAN service discovery, поддерживается на всех платформах (Avahi на Linux, Bonjour на macOS, Windows). Alternative — broadcast UDP с custom protocol — пришлось бы делать сами, без выигрыша.

### Почему lefthook, а не husky / pre-commit?

- **husky** — JS-based, медленнее (overhead Node startup), не parallel.
- **pre-commit** (Python) — отличный, но добавляет Python deps в Rust-heavy репо.
- **lefthook** — Go binary, ~5MB, parallel execution, простой YAML конфиг. Быстрее husky в 3-10×.

### Почему VitePress, а не Docusaurus / MkDocs / mdBook?

- **Docusaurus** (Facebook) — React, тяжёлый, Markdown-only.
- **MkDocs** (Python) — простой, но slow build, плагины слабые.
- **mdBook** (Rust) — Rust ecosystem, но Markdown-only, нет Vue компонентов в страницах.
- **VitePress** — Vite-native, Vue компоненты прямо в md, fast HMR. Подходит к нашему стеку идеально.

### Почему Storybook 10, а не histoire?

Был параллельный histoire — удалён 2026-05-19. Storybook тяжелее (~200MB deps vs ~30MB у histoire), но **экосистема больше**: accessibility addon, vitest integration, MDX docs, viewport addon — всё нужное «искаропки». Histoire были Vue-only и легче, но дублирование двух систем не оправдывало overhead поддержки.

### Почему better-sqlite3 в Electron, а не node:sqlite?

`node:sqlite` (built-in в Node 22+) — реальная альтернатива. Electron 41 использует Node 22.x — миграция возможна, **уберёт native dep** (electron-rebuild), уменьшит installer. **На watch-list** — в Phase 11 (backup feature) пересмотрим.

### Почему ARK FTS5, а не tantivy / meilisearch / sqlite-vec semantic?

- **tantivy** — Rust Lucene-like, мощнее FTS5, но separate process, no embedded API.
- **meilisearch** — external service, требует daemon.
- **sqlite-vec** (semantic embeddings) — в [Phase 16 roadmap](/apps/kepler-roadmap), будет работать **поверх** существующего ARK FTS5, не вместо. Hybrid lexical+semantic search.

FTS5 встроен в SQLite, синхронизируется sync'ом ARK автоматически, нет separate process — для local-first что нужно.
